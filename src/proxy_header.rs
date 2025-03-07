use std::{error::Error, net::SocketAddr};

#[allow(dead_code)]
pub enum Protocol {
    TCP = 0x01,
    UDP = 0x02
}

pub fn generate_proxy_header(protocol: Protocol, source: SocketAddr, dest: SocketAddr) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut header: Vec<u8> = vec![
        0x0D, 0x0A, 0x0D, 0x0A, 0x00, 0x0D, 0x0A, 0x51, 0x55, 0x49, 0x54, 0x0A, 0x21
    ];

    if (source.is_ipv4() && dest.is_ipv6()) || (source.is_ipv6() && dest.is_ipv4()) {
        return Err("Source and destination IP addresses must be of the same type".into());
    }

    match source {
        SocketAddr::V4(ip) => {
            header.push(0x10 | protocol as u8);
            header.extend(ip.ip().octets().iter());
        }
        SocketAddr::V6(ip) => {
            header.push(0x20 | protocol as u8);
            header.extend(ip.ip().octets().iter());
        }
    }

    match dest {
        SocketAddr::V4(ip) => {
            header.extend(ip.ip().octets().iter());
        }
        SocketAddr::V6(ip) => {
            header.extend(ip.ip().octets().iter());
        }
    }

    header.extend(source.port().to_be_bytes());
    header.extend(dest.port().to_be_bytes());

    Ok(header)
}
