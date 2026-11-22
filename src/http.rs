use crate::error::{Error, Result};
use crate::log::Log;
use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

const TLS_FALLBACK_WARNING: &str = "El CDN de Sony usa un certificado de la CA privada SCEI DNAS Root 05. Reintentando sin verificar TLS (equivale a curl -k). Las piezas se siguen verificando con SHA-256.";

fn agent(insecure: bool, error_code: i32) -> Result<ureq::Agent> {
    let builder = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(30))
        .timeout_read(Duration::from_secs(120))
        .timeout_write(Duration::from_secs(120))
        .redirects(5)
        .user_agent(concat!("pkg-merge/", env!("CARGO_PKG_VERSION")));

    let builder = if insecure {
        // El CDN de Sony usa una CA privada (SCEI DNAS Root 05) que no está en
        // el almacén del sistema. Solo se desactiva la validación con este flag.
        let connector = ureq::native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .map_err(|e| {
                Error::new(
                    error_code,
                    format!("no se pudo crear el conector TLS inseguro: {e}"),
                )
            })?;
        builder.tls_connector(Arc::new(connector))
    } else {
        builder
    };
    Ok(builder.build())
}

fn is_tls_error(error: &ureq::Error) -> bool {
    match error {
        ureq::Error::Transport(transport) => {
            let text = transport.to_string().to_ascii_lowercase();
            text.contains("certificate")
                || text.contains("certificado")
                || text.contains("tls")
                || text.contains("ssl")
                || text.contains("handshake")
        }
        _ => false,
    }
}

pub fn get_bytes(
    url: &str,
    max_bytes: u64,
    error_code: i32,
    insecure: bool,
    log: &Log,
) -> Result<Vec<u8>> {
    let client = agent(insecure, error_code)?;
    let response = match client.get(url).call() {
        Ok(response) => response,
        Err(error) if !insecure && is_tls_error(&error) => {
            log.warn(TLS_FALLBACK_WARNING);
            agent(true, error_code)?
                .get(url)
                .call()
                .map_err(|retry| map_error(retry, error_code, url, true))?
        }
        Err(error) => return Err(map_error(error, error_code, url, insecure)),
    };
    let mut reader = response.into_reader().take(max_bytes + 1);
    let mut data = Vec::new();
    reader
        .read_to_end(&mut data)
        .map_err(|e| Error::new(error_code, format!("error leyendo {url}: {e}")))?;
    if data.len() as u64 > max_bytes {
        return Err(Error::new(
            error_code,
            format!("la respuesta de {url} supera el límite de {max_bytes} bytes"),
        ));
    }
    Ok(data)
}

pub struct Download {
    pub len: Option<u64>,
    pub reader: Box<dyn Read + Send + Sync + 'static>,
}

pub fn get(url: &str, error_code: i32, insecure: bool, log: &Log) -> Result<Download> {
    let client = agent(insecure, error_code)?;
    let response = match client.get(url).call() {
        Ok(response) => response,
        Err(error) if !insecure && is_tls_error(&error) => {
            log.warn(TLS_FALLBACK_WARNING);
            agent(true, error_code)?
                .get(url)
                .call()
                .map_err(|retry| map_error(retry, error_code, url, true))?
        }
        Err(error) => return Err(map_error(error, error_code, url, insecure)),
    };
    let len = response
        .header("Content-Length")
        .and_then(|value| value.parse::<u64>().ok());
    Ok(Download {
        len,
        reader: response.into_reader(),
    })
}

fn map_error(error: ureq::Error, code: i32, url: &str, insecure: bool) -> Error {
    let message = match error {
        ureq::Error::Status(status, _) => format!("HTTP {status} al descargar {url}"),
        ureq::Error::Transport(transport) => {
            format!("error de red al descargar {url}: {transport}")
        }
    };
    let lower = message.to_ascii_lowercase();
    let looks_like_tls = lower.contains("certificate")
        || lower.contains("certificado")
        || lower.contains("tls")
        || lower.contains("ssl")
        || lower.contains("handshake");
    if !insecure && looks_like_tls {
        Error::new(
            code,
            format!(
                "{message}\n\nEl CDN de Sony usa una CA privada (SCEI DNAS Root 05) que Windows/macOS/Linux no trustan por defecto.\n\
                 Opciones:\n\
                 1) Deja que pkg-merge reintente solo (lo hace automáticamente).\n\
                 2) Fuerza el modo inseguro con --insecure-tls.\n\
                 3) Descarga el manifiesto .json y pásalo como archivo local."
            ),
        )
    } else {
        Error::new(code, message)
    }
}
