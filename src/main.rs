mod commands;
mod resp;

use crate::commands::types::ResponseError;
use crate::commands::{echo, ping};
use crate::resp::types::{ParseError, RESPValue};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn handle_connection(
    mut stream: tokio::net::TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    // read request
    let mut buf = Vec::new();
    loop {
        let r: Option<RESPValue>;
        loop {
            let mut tmp = [0u8; 4096];
            let n = stream.read(&mut tmp).await?;
            if n == 0 {
                return Ok(()); // connection closed
            }
            buf.extend_from_slice(&tmp[..n]);
            let s = str::from_utf8(&buf)?;

            match s.parse::<RESPValue>() {
                Ok(val) => {
                    r = Some(val);
                    buf.clear(); // clear buffer after successful parse for next requests under the same connection
                    break;
                }
                Err(e) => {
                    match e {
                        ParseError::Incomplete => continue, // continue processing bytes
                        ParseError::Invalid => return Err(Box::new(e)),
                    }
                }
            }
        }

        // match proccess with RESP
        let raw_resp: RESPValue;
        match &r {
            Some(RESPValue::Array(a)) => {
                if a.is_empty() {
                    return Err(Box::new(ResponseError::MalformedRequestError));
                }

                match &a[0] {
                    RESPValue::BulkString(cmd) => {
                        if cmd.eq_ignore_ascii_case("echo") {
                            raw_resp = echo::echo(a)?;
                        } else if cmd.eq_ignore_ascii_case("ping") {
                            raw_resp = ping::ping()?;
                        } else {
                            return Err(Box::new(ResponseError::UnsupportedCommandError));
                        }
                    }
                    _ => {
                        return Err(Box::new(ResponseError::MalformedRequestError));
                    }
                }
            }
            Some(_) => return Err(Box::new(ResponseError::MalformedRequestError)),
            _ => return Err(Box::new(ResponseError::MalformedRequestError)),
        }

        // write response
        let resp_bytes = raw_resp.to_string().into_bytes();
        stream.write_all(&resp_bytes).await?;
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReadConfigError;

struct Config {
    timeout: std::time::Duration,
}

async fn read_config() -> Result<Config, ReadConfigError> {
    let timeout_ms = std::env::var("CONN_TIMEOUT")
        .ok()
        .and_then(|t| t.parse::<u64>().ok())
        .unwrap_or(0);

    Ok(Config {
        timeout: std::time::Duration::from_millis(timeout_ms),
    })
}

#[tokio::main]
async fn main() {
    // join! may be premature optimization, but i plan on file-read in the future and don't want to forget this is a thing...
    let (config_res, listener_res) = tokio::join!(
        read_config(),
        tokio::net::TcpListener::bind("127.0.0.1:6379")
    );
    let config = config_res.unwrap();
    let listener = listener_res.unwrap();

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            if config.timeout.is_zero() {
                let err = handle_connection(stream).await.err();
                if !err.is_none() {
                    // todo handle error? log?
                }
            } else {
                tokio::select! {
                    _ = handle_connection(stream) => {}
                    _ = tokio::time::sleep(config.timeout) => {
                        // stream.shutdown().await; how to safely handle a shutdown?
                    }
                }
            }
        });
    }
}
