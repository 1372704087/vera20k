//! Small UDP-only relay. No retail assets, simulation engine, client identity
//! database or lobby bearer credentials are present in this process.

use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use vera20k_relay_protocol::{MAX_DATAGRAM, RelayServer, SecretKey};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = load_key()?;
    let bind = std::env::var("VERA_RELAY_BIND").unwrap_or_else(|_| "0.0.0.0:5000".to_string());
    let socket = UdpSocket::bind(&bind)?;
    socket.set_read_timeout(Some(Duration::from_secs(1)))?;
    let mut server = RelayServer::new(SecretKey::new(key), 1024);
    let mut buffer = [0u8; MAX_DATAGRAM + 1];
    eprintln!("VERA20k relay listening on {}", socket.local_addr()?);
    loop {
        let (length, source) = match socket.recv_from(&mut buffer) {
            Ok(packet) => packet,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let SocketAddr::V4(source) = source else {
            continue;
        };
        if length > MAX_DATAGRAM {
            continue;
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        // Rejections expose no packet, key, ticket, address or player data.
        // Ordinary UDP loss is handled by the existing client reliable owner.
        if let Ok(forward) = server.process(source, &buffer[..length], now) {
            for datagram in forward {
                let _ = socket.send_to(&datagram.bytes, datagram.endpoint);
            }
        }
    }
}

fn load_key() -> Result<[u8; 32], Box<dyn std::error::Error>> {
    // systemd LoadCredential keeps signing material out of environment values
    // and process arguments. A selected credential file must be readable;
    // failure never falls back to a different secret source.
    let path = std::env::var_os("VERA_RELAY_TICKET_KEY_FILE")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CREDENTIALS_DIRECTORY")
                .map(|directory| PathBuf::from(directory).join("relay-ticket-key"))
        });
    let text = if let Some(path) = path {
        std::fs::read_to_string(path).map_err(|_| "unable to read relay signing credential file")?
    } else {
        // Local development only; deployed units use LoadCredential above.
        std::env::var("VERA_RELAY_TICKET_KEY")
            .map_err(|_| "relay signing credential file is not configured")?
    };
    Ok(decode_key(text.trim())?)
}

fn decode_key(text: &str) -> Result<[u8; 32], &'static str> {
    if text.len() != 64 {
        return Err("relay signing key must be exactly 64 hexadecimal digits");
    }
    let mut key = [0u8; 32];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        fn digit(byte: u8) -> Result<u8, &'static str> {
            match byte {
                b'0'..=b'9' => Ok(byte - b'0'),
                b'a'..=b'f' => Ok(byte - b'a' + 10),
                b'A'..=b'F' => Ok(byte - b'A' + 10),
                _ => Err("relay signing key must contain only hexadecimal digits"),
            }
        }
        key[index] = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    Ok(key)
}
