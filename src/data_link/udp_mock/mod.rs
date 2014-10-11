use std::collections::hashmap::{HashMap, Occupied, Vacant};

use std::io::{IoError, IoResult, IoUnavailable};
use std::io::net;
use std::io::net::udp::UdpSocket;
use std::io::net::ip::{SocketAddr, Ipv4Addr, Port};

use std::sync::Arc;
use std::sync::RWLock;

use std::task::spawn;

use interface::Interface as Interface_T;

use data_link::{DLPacket, DLHandler, DLInterface};

#[cfg(test)]
mod test;


const RECV_BUF_SIZE: uint = 64 * 1024;

type SharedHandlerMap = Arc<RWLock<HashMap<SocketAddr, ( bool, DLHandler)>>>;

/// The backing listening socket / read loop for a bunch of UDP-backed mock link ip_to_interface
pub struct Listener {
    socket:   UdpSocket,
    handlers: SharedHandlerMap,
}

impl Listener {

    pub fn new(listen_addr: SocketAddr, num_threads: uint) -> IoResult<Listener>
    {

        assert!(num_threads > 0);

        let socket = try!(UdpSocket::bind(listen_addr));

        let handlers: SharedHandlerMap = Arc::new(RWLock::new(HashMap::new()));


        for _ in range(0, num_threads) {
            let mut socket   = socket.clone();
            let     handlers = handlers.clone();
            spawn(proc() {
                // TODO: shouldn't need to initialize this
                let mut buf: [u8, ..RECV_BUF_SIZE] = [0, ..RECV_BUF_SIZE];
                loop {
                    // select! (
                    //     () = die.recv() => {
                    //         println!("I am going to die");
                    //         return;
                    //     },
                    match socket.recv_from(buf.as_mut_slice()) {
                        Err(_) => {
                            // maybe it will work next time...
                            println!("something bad happened");
                        },
                        Ok((len, src_addr)) => match handlers.read().deref().find(&src_addr) {
                            None          => continue, // drop that packet!
                            Some(&(is_enabled, ref on_recv)) => {
                                println!("received packet");
                                if is_enabled {
                                    println!("Interface is enabled");
                                    let args = buf[..len].to_vec();
                                    (**on_recv).call((args,));
                                }
                            },
                        }
                    };
                    // )
                }
            });
        }

        Ok(Listener {
            socket:   socket,
            handlers: handlers,
        })
    }
}

impl Clone for Listener {

    fn clone(&self) -> Listener {
        Listener {
            socket:   self.socket  .clone(),
            handlers: self.handlers.clone(),
        }
    }
}


/// A mock link layer interface made from UDP
pub struct Interface {
    listener:    Listener,
    remote_addr: SocketAddr,
    cached_status: bool,
}


impl Interface {

    pub fn new(listener:    &Listener,
               remote_addr: SocketAddr,
               on_recv:     DLHandler) -> Interface
    {
        listener.handlers.write().deref_mut().insert(remote_addr, (true, on_recv));

        Interface {
            listener:      listener.clone(),
            remote_addr:   remote_addr,
            cached_status: true,
        }
    }
}

impl Interface_T for Interface {

}

const DISABLED_INTERFACE_ERROR: IoError = IoError {
    kind: IoUnavailable,
    desc: "This link-layer interface (a Interface) has been disabled",
    detail: None,
};

impl DLInterface for Interface {


    fn send(&self, packet: DLPacket) -> IoResult<()> {

        if self.cached_status == false {
            return Err(DISABLED_INTERFACE_ERROR.clone())
        }

        let mut socket = self.listener.socket.clone();
        socket.send_to(packet.as_slice(), self.remote_addr)
    }

    fn update_recv_handler(&self, on_recv: DLHandler) {
        self.listener.handlers.write().deref_mut()
            .insert(self.remote_addr, (true, on_recv));
    }

    fn enable(&mut self) {
        let mut map = self.listener.handlers.write();
        self.cached_status = true;
        match map.deref_mut().entry(self.remote_addr) {
            Vacant(_) => fail!("udp mock interface should already have entry in table"),
            Occupied(mut entry) => {
                let &(ref mut status, _) = entry.get_mut();
                *status = true;
            },
        }
    }

    fn disable(&mut self) {
        let mut map = self.listener.handlers.write();
        self.cached_status = false;
        match map.deref_mut().entry(self.remote_addr) {
            Vacant(_) => fail!("udp mock interface should already have entry in table"),
            Occupied(mut entry) => {
                let &(ref mut status, _) = entry.get_mut();
                *status = false;
            },
        }
    }

}
