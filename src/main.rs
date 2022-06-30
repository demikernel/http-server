use ::demikernel::{LibOS, OperationResult, QDesc, QToken};
use std::fs;
use ::std::net::{Ipv4Addr, SocketAddrV4};
use ::runtime::memory::Buffer;

fn main() {
    let mut libos: LibOS = LibOS::new();

    // Create listening socket
    let local = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 7878);
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
        
    // create list of qtokens
    let mut qtokens: Vec<QToken> = Vec::new();

    // Wait for first connection
    let accept_qt: QToken = match libos.accept(sockqd) {
        Ok(qt) => qt,
        Err(e) => panic!("failed to accept connection on socket: {:?}", e.cause),
    };
    qtokens.push(accept_qt);

    // loop over connections
    loop {
        let (i, qd, result) = match libos.wait_any2(&qtokens) {
            Ok((i, qd, result)) => (i, qd, result),
            Err(e) => panic!("Wait failed: {:?}", e),
        };
        
        let qt = match result {
            // New connection
            OperationResult::Accept(qd) => {
                // pop from new connection
                let qt: QToken = match libos.pop(qd) {
                    Ok(qt) => qt,
                    Err(e) => panic!("failed to pop data from socket: {:?}", e.cause),
                };
                qtokens.push(qt);
                println!("Connection established!");
                // accept next connection
                match libos.accept(sockqd) {
                    Ok(qt) => qt,
                    Err(e) => panic!("failed to accept connection on socket: {:?}", e.cause),
                }
            }
            // Pop
            OperationResult::Pop(_, buf) => {
                // process buffer
                let response = process_request(buf);
                match libos.push2(qd, &response) {
                    Ok(qt) => qt,
                    Err(e) => panic!("failed to push data to socket: {:?}", e.cause),
                }       
                // match libos.wait2(qt) {
                //     Ok((_, OperationResult::Push)) => (),
                //     Err(e) => panic!("Push response failed: {:?}", e.cause),
                // };
            }
            // Push
            OperationResult::Push => {
                // pop again
                match libos.pop(qd) {
                    Ok(qt) => qt,
                    Err(e) => panic!("failed to pop data from socket: {:?}", e.cause),
                }
            }
            OperationResult::Failed(e) => panic!("operation failed: {:?}", e),
            _ => panic!("unexpected result"),
        };

        qtokens[i] = qt;
    }
}

fn process_request(buffer: Box<dyn Buffer>) -> Vec<u8> {
    println!("Request: {}", String::from_utf8_lossy(&buffer[..]));
    let contents = fs::read_to_string("hello.html").unwrap();

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
        contents.len(),
        contents
    );
    response.as_bytes().to_vec()
}
