// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

// A (Very) Simple HTTP Server.

use ::demikernel::{
    demi_sgarray_t,
    runtime::{
        fail::Fail,
        types::demi_opcode_t,
    },
    LibOS,
    LibOSName,
    QDesc,
    QToken,
};
use ::std::{
    collections::HashMap,
    fs,
    io::Write,
    net::{
        Ipv4Addr,
        SocketAddrV4,
    },
    slice,
};

fn main() -> ! {
    // Pull LibOS from environment variable "LIBOS" if present.  Otherwise, default to Catnap.
    let libos_name: LibOSName = match LibOSName::from_env() {
        Ok(libos_name) => libos_name.into(),
        Err(_) => LibOSName::Catnap,
    };

    // Initialize the LibOS.
    let mut libos: LibOS = match LibOS::new(libos_name) {
        Ok(libos) => libos,
        Err(e) => panic!("failed to initialize libos: {:?}", e.cause),
    };

    // Create listening socket (a QDesc in Demikernel).
    let local_addr: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 7878);
    let listening_qd: QDesc = match create_listening_socket(&mut libos, local_addr) {
        Ok(qd) => qd,
        Err(e) => panic!("create_listening_socket failed: {:?}", e.cause),
    };

    println!("Listening on local address: {:?}", local_addr);

    // Create list of queue tokens (QToken) representing operations we're going to wait on.
    let mut waiters: Vec<QToken> = Vec::new();

    // Post an accept for the first connection.
    let accept_qt: QToken = match libos.accept(listening_qd) {
        Ok(qt) => qt,
        Err(e) => panic!("failed to accept connection on socket: {:?}", e.cause),
    };
    // Add this accept to the list of operations we're waiting to complete.
    waiters.push(accept_qt);

    // Create hash table of accepted connections.
    let mut connections: HashMap<QDesc, Connection> = HashMap::new();

    // Loop over queue tokens we're waiting on.
    loop {
        let (index, qr) = match libos.wait_any(&waiters, None) {
            Ok((i, qr)) => (i, qr),
            Err(e) => panic!("Wait failed: {:?}", e),
        };

        // Since this QToken completed, remove it from the list of waiters.
        waiters.swap_remove(index);

        // Find out what operation completed:
        match qr.qr_opcode {
            demi_opcode_t::DEMI_OPC_ACCEPT => {
                // A new connection arrived.
                let qd: QDesc = unsafe { qr.qr_value.ares.qd.into() };
                println!("Connection established!  Queue Descriptor = {:?}", qd);

                // Create new connection state and store it.
                let connection: Connection = Connection::new(qd);
                connections.insert(qd, connection);

                // Post a pop from the new connection.
                match libos.pop(qd) {
                    Ok(qt) => waiters.push(qt),
                    Err(e) => panic!("failed to pop data from socket: {:?}", e.cause),
                };

                // Post an accept for the next connection.
                match libos.accept(listening_qd) {
                    Ok(qt) => waiters.push(qt),
                    Err(e) => panic!("failed to accept connection on socket: {:?}", e.cause),
                }
            },
            demi_opcode_t::DEMI_OPC_POP => {
                // A pop completed.
                let qd: QDesc = qr.qr_qd.into();
                let recv_sga: demi_sgarray_t = unsafe { qr.qr_value.sga };

                // Find Connection for this queue descriptor.
                let connection: &mut Connection = connections.get_mut(&qd).expect("HashMap should hold connection!");

                // Process the incoming request.
                let oqt: Option<QToken> = match connection.process_data(&mut libos, recv_sga) {
                    Ok(oqt) => oqt,
                    Err(e) => panic!("process data failed: {:?}", e.cause),
                };

                if oqt.is_some() {
                    waiters.push(oqt.expect("oqt should be some!"));
                }

                // Post another pop.
                match libos.pop(qd) {
                    Ok(qt) => waiters.push(qt),
                    Err(e) => panic!("failed to pop data from socket: {:?}", e.cause),
                }
            },
            demi_opcode_t::DEMI_OPC_PUSH => {
                // A push completed.
                let _qd: QDesc = qr.qr_qd.into();

                // ToDo: Can free the sga now.
            },
            demi_opcode_t::DEMI_OPC_FAILED => panic!("operation failed"),
            _ => panic!("unexpected opcode"),
        };
    }
}

// Creates a listening socket with the given IPv4 address/port and returns the queue descriptor for it.
fn create_listening_socket(libos: &mut LibOS, address: SocketAddrV4) -> Result<QDesc, Fail> {
    // Create the socket.
    let listening_qd: QDesc = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0)?;

    // Bind to local address.
    libos.bind(listening_qd, address)?;

    // Mark socket as a passive (i.e. listening) one.
    const BACKLOG: usize = 16;
    libos.listen(listening_qd, BACKLOG)?;

    Ok(listening_qd)
}

// The Connection type tracks connection state.
struct Connection {
    queue_descriptor: QDesc,
    receive_queue: Vec<demi_sgarray_t>,
}

impl Connection {
    pub fn new(qd: QDesc) -> Self {
        Connection {
            queue_descriptor: qd,
            receive_queue: Vec::new(),
        }
    }

    pub fn process_data(&mut self, libos: &mut LibOS, rsga: demi_sgarray_t) -> Result<Option<QToken>, Fail> {
        // We only handle single-segment scatter-gather arrays for now.
        assert_eq!(rsga.sga_numsegs, 1);

        // Print the incoming data.
        let slice: &mut [u8] = unsafe {
            slice::from_raw_parts_mut(
                rsga.sga_segs[0].sgaseg_buf as *mut u8,
                rsga.sga_segs[0].sgaseg_len as usize,
            )
        };
        println!("Received: {}", String::from_utf8_lossy(slice));

        // Add incoming scatter-gather segment to our receive queue.
        self.receive_queue.push(rsga);

        // Craft a response and send it.
        let ssga: demi_sgarray_t = libos.sgaalloc(1500)?;
        let mut slice: &mut [u8] = unsafe {
            slice::from_raw_parts_mut(
                ssga.sga_segs[0].sgaseg_buf as *mut u8,
                ssga.sga_segs[0].sgaseg_len as usize,
            )
        };

        // Read the entire file into a string.
        let contents: String = fs::read_to_string("hello.html").expect("file should exist and be readable.");

        write!(
            slice,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            contents.len(),
            contents
        )?;

        // Write response.
        let qt = libos.push(self.queue_descriptor, &ssga)?;

        Ok(Some(qt))
    }
}
