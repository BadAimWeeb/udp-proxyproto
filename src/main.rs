use std::{
    collections::HashMap,
    env,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use futures::future;
use tokio::{
    net::UdpSocket,
    sync::{mpsc::Sender, Mutex},
    task::JoinHandle
};

mod proxy_header;

#[tokio::main]
async fn main() {
    // This code is highly inefficient, i know, but it's just a proof of concept for now.

    let args: Vec<String> = env::args().collect();
    let listen_port = args[1].parse::<u16>().unwrap();
    let target_addr = args[2].parse::<SocketAddr>().unwrap();

    let udp6_server = Arc::new(UdpSocket::bind(("::", listen_port)).await.unwrap());
    let udp4_server = UdpSocket::bind(("0.0.0.0", listen_port)).await;

    let socket_map: HashMap<SocketAddr, (JoinHandle<()>, Sender<Vec<u8>>)> = HashMap::new();
    let mutex_socket_map = Arc::new(Mutex::new(socket_map));

    // If socket is not used for 30 seconds, close it
    let timeout: u128 = 30000;

    let msm_udp4 = mutex_socket_map.clone();
    let udp4_thread = tokio::spawn(async move {
        if udp4_server.is_err() {
            return;
        }
        let udp4_server = Arc::new(udp4_server.unwrap());

        let mutex_socket_map = msm_udp4;

        let server_addr = udp4_server.local_addr().unwrap();
        println!("Listening on {} (IPv4)", server_addr);

        let mut buf = [0u8; 65535];

        let udp4_recv = udp4_server.clone();
        loop {
            let (len, src) = udp4_recv.recv_from(&mut buf).await.unwrap();
            let mut sm = mutex_socket_map.lock().await;
            if let Some((handle, sender)) = sm.get_mut(&src) {
                if !handle.is_finished() {
                    sender.send(buf[..len].to_vec()).await.unwrap();
                    continue;
                }
            }

            let (sender, mut receiver) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
            let target_addr = target_addr.clone();

            let last_io = Arc::new(Mutex::new(get_current_unix()));
            let udp4_send = udp4_server.clone();
            let handle = tokio::spawn(async move {
                let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await.unwrap());
                socket.connect(target_addr).await.unwrap();
                let socket_addr = socket.local_addr().unwrap();
                println!("Establishing connection for {} (routed using {})", src, socket_addr);
                socket.send(&proxy_header::generate_proxy_header(proxy_header::Protocol::UDP, src, SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), server_addr.port()))).unwrap()).await.unwrap();

                let lio_send = last_io.clone();
                let socket_send = socket.clone();
                let handle_send = tokio::spawn(async move {
                    let socket = socket_send;
                    loop {
                        let data_chunk = receiver.recv().await.unwrap();
                        socket.send(&data_chunk).await.unwrap();

                        let mut lio = lio_send.lock().await;
                        *lio = get_current_unix();
                    }
                });

                let lio_recv = last_io.clone();
                let socket_recv = socket.clone();
                let handle_recv = tokio::spawn(async move {
                    let mut buf = [0u8; 65535];
                    let socket = socket_recv;
                    loop {
                        let (len, _) = socket.recv_from(&mut buf).await.unwrap();
                        udp4_send.send_to(&buf[..len], src).await.unwrap();

                        let mut lio = lio_recv.lock().await;
                        *lio = get_current_unix();
                    }
                });

                let handle_kill = tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                        let lio = last_io.lock().await;
                        if get_current_unix() - *lio > timeout {
                            break;
                        }
                    }
                });

                let _ = handle_kill.await;
                handle_send.abort();
                handle_recv.abort();

                println!("Connection for {} closed (was routed using {})", src, socket_addr);
            });

            sender.send(buf[..len].to_vec()).await.unwrap();
            sm.insert(src, (handle, sender));
        }
    });

    let msm_udp6 = mutex_socket_map.clone();
    let udp6_thread = tokio::spawn(async move {
        let mutex_socket_map = msm_udp6;

        let server_addr = udp6_server.local_addr().unwrap();
        println!("Listening on {} (IPv6)", server_addr);

        let mut buf = [0u8; 65535];

        let udp6_recv = udp6_server.clone();
        loop {
            let (len, src) = udp6_recv.recv_from(&mut buf).await.unwrap();
            let mut sm = mutex_socket_map.lock().await;
            if let Some((handle, sender)) = sm.get_mut(&src) {
                if !handle.is_finished() {
                    sender.send(buf[..len].to_vec()).await.unwrap();
                    continue;
                }
            }

            let (sender, mut receiver) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
            let target_addr = target_addr.clone();

            let last_io = Arc::new(Mutex::new(get_current_unix()));
            let udp6_send = udp6_server.clone();
            let handle = tokio::spawn(async move {
                let socket = Arc::new(UdpSocket::bind(":::0").await.unwrap());
                socket.connect(target_addr).await.unwrap();
                let socket_addr = socket.local_addr().unwrap();
                println!("Establishing connection for {} (routed using {})", src, socket_addr);
                socket.send(&proxy_header::generate_proxy_header(proxy_header::Protocol::UDP, src, SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::from_bits(1), server_addr.port(), 0, 0))).unwrap()).await.unwrap();

                let lio_send = last_io.clone();
                let socket_send = socket.clone();
                let handle_send = tokio::spawn(async move {
                    let socket = socket_send;
                    loop {
                        let data_chunk = receiver.recv().await.unwrap();
                        socket.send(&data_chunk).await.unwrap();

                        let mut lio = lio_send.lock().await;
                        *lio = get_current_unix();
                    }
                });

                let lio_recv = last_io.clone();
                let socket_recv = socket.clone();
                let handle_recv = tokio::spawn(async move {
                    let mut buf = [0u8; 65535];
                    let socket = socket_recv;
                    loop {
                        let (len, _) = socket.recv_from(&mut buf).await.unwrap();
                        udp6_send.send_to(&buf[..len], src).await.unwrap();

                        let mut lio = lio_recv.lock().await;
                        *lio = get_current_unix();
                    }
                });

                let handle_kill = tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                        let lio = last_io.lock().await;
                        if get_current_unix() - *lio > timeout {
                            break;
                        }
                    }
                });

                let _ = handle_kill.await;
                handle_send.abort();
                handle_recv.abort();

                println!("Connection for {} closed (was routed using {})", src, socket_addr);
            });

            sender.send(buf[..len].to_vec()).await.unwrap();
            sm.insert(src, (handle, sender));
        }
    });

    let _ = future::join(udp4_thread, udp6_thread).await;
}

fn get_current_unix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}
