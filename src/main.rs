use ::demikernel::{LibOS, OperationResult, QDesc, QToken};
use std::fs;
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;

fn main() {
    let libos: LibOS = LibOS::new();

    // Create listening socket
    let listening_sockqd: QDesc = setup("127.0.0.1:7878");
    // create list of qtokens
    let mut qtokens: Vec<QToken> = Vec::new();

    // Wait for first connection
    let accept_qt: QToken = match libos.accept(listening_sockqd) {
        Ok(qt) => qt,
        Err(e) => panic!("failed to accept connection on socket: {:?}", e.cause),
    };
    qtokens.push(qt);

    // loop over connections
    loop {
        let (i, qd, result) = match self.libos.wait_any2(&qtokens) {
            Ok((i, qd, result)) => (i, qd, result),
            Err(e) => panic!("Wait failed: {:?}", e),
        };
        
        match result {
            OperationResult::Accept(qd) => {
                // accept next connection
                accept_qt: QToken = match libos.accept(listening_sockqd) {
                    Ok(qt) => qt,
                    Err(e) => panic!("failed to accept connection on socket: {:?}", e.cause),
                };
                qtokens[i] = accept_qt; 

                // pop from new connection
                let qt: QToken = match libos.pop(qd) {
                    Ok(qt) => qt,
                    Err(e) => panic!("failed to pop data from socket: {:?}", e.cause),
                };
                qtokens.push(qt);
                println!("Connection established!");
            }

        }
                handle_request();

    }
}

fn setup(local: String) -> QDesc {
    let sockqd: QDesc = match libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0) {
        Ok(qd) => qd,
        Err(e) => panic!("failed to create socket: {:?}", e.cause),
    };

    // Bind to local address
    match libos.bind(sockqd, local) {
        Ok(()) => (),
        Err(e) => panic!("failed to bind socket: {:?}", e.cause),
    };

    // Mark socket as a passive one.
    match libos.listen(sockqd, 16) {
        Ok(()) => (),
        Err(e) => panic!("failed to listen socket: {:?}", e.cause),
    }

    println!("Local Address: {:?}", local);
    sockqd
}

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0; 1024];

    stream.read(&mut buffer).unwrap();
    let contents = fs::read_to_string("hello.html").unwrap();

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
        contents.len(),
        contents
    );

    stream.write(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}
