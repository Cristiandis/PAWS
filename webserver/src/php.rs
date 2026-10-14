use std::io::{Read, Write};
use std::net::TcpStream;

const BEGIN: u8 = 1;
const END: u8 = 3;
const PARAMS: u8 = 4;
const STDIN: u8 = 5;
const STDOUT: u8 = 6;
const STDERR: u8 = 7;

fn write_record(out: &mut Vec<u8>, ty: u8, content: &[u8]) {
    let id = 1u16.to_be_bytes();
    if content.is_empty() {
        out.extend_from_slice(&[1, ty, id[0], id[1], 0, 0, 0, 0]);
        return;
    }
    for chunk in content.chunks(65535) {
        let len = (chunk.len() as u16).to_be_bytes();
        out.extend_from_slice(&[1, ty, id[0], id[1], len[0], len[1], 0, 0]);
        out.extend_from_slice(chunk);
    }
}

fn encode_len(out: &mut Vec<u8>, n: usize) {
    if n < 128 {
        out.push(n as u8);
    } else {
        out.extend_from_slice(&((n as u32) | 0x8000_0000).to_be_bytes());
    }
}

pub fn fastcgi_request(
    addr: &str,
    params: &[(String, String)],
    body: &[u8],
) -> anyhow::Result<Vec<u8>> {
    let mut stream = TcpStream::connect(addr)?;
    let mut buf = Vec::new();

    // BEGIN_REQUEST: role = 1 (responder), flags = 0
    write_record(&mut buf, BEGIN, &[0, 1, 0, 0, 0, 0, 0, 0]);

    let mut p = Vec::new();
    for (k, v) in params {
        encode_len(&mut p, k.len());
        encode_len(&mut p, v.len());
        p.extend_from_slice(k.as_bytes());
        p.extend_from_slice(v.as_bytes());
    }
    write_record(&mut buf, PARAMS, &p);
    write_record(&mut buf, PARAMS, &[]);
    write_record(&mut buf, STDIN, body);
    write_record(&mut buf, STDIN, &[]);
    stream.write_all(&buf)?;

    let mut stdout = Vec::new();
    loop {
        let mut h = [0u8; 8];
        stream.read_exact(&mut h)?;
        let len = u16::from_be_bytes([h[4], h[5]]) as usize;
        let pad = h[6] as usize;
        let mut content = vec![0u8; len + pad];
        stream.read_exact(&mut content)?;
        content.truncate(len);
        match h[1] {
            STDOUT => stdout.extend_from_slice(&content),
            STDERR => eprintln!("php-fpm: {}", String::from_utf8_lossy(&content)),
            END => break,
            _ => {}
        }
    }
    Ok(stdout)
}
