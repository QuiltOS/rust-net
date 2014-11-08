use std::io::IoResult;
use std::io::net::ip::{SocketAddr, IpAddr, Ipv4Addr, Port};
use std::io::net::udp::UdpSocket;

use std::sync::{Arc, Barrier};
use std::str::from_utf8;
use std::string::String;

use misc::interface::{MyFn, SenderClosure, Nop};

use super::*;

fn mk_listener(num_threads: uint) -> IoResult<(Listener, SocketAddr)> {
    // port 0 is dynamically assign
    let mut listener = try!(Listener::new(SocketAddr { ip: Ipv4Addr(0,0,0,0), port: 0 },
                                          num_threads));
    let mut addr     = try!(listener.socket.socket_name());
    addr.ip = Ipv4Addr(127, 0, 0, 1);
    println!("made listener with addr: {}", addr);
    Ok((listener, addr))
}

fn talk_to_self_channel_helper(num_threads: uint) {
    use std::comm;

    fn inner(num_threads: uint) -> IoResult<()> {
        let (l1, a1) = try!(mk_listener(num_threads));
        let (l2, a2) = try!(mk_listener(num_threads));

        let (tx1, rx1) = channel::<(::Packet,)>();
        let (tx2, rx2) = channel::<(::Packet,)>();

        const M1: &'static str = "Hey Josh!";
        const M2: &'static str = "Hey Cody!";

        let interface1 = Interface::new(&l1, a2, box SenderClosure::new(tx1));
        let interface2 = Interface::new(&l2, a1, box SenderClosure::new(tx2));

        try!(::Interface::send(&interface1, String::from_str(M2).into_bytes()));
        try!(::Interface::send(&interface2, String::from_str(M1).into_bytes()));

        let (packet_1,) = rx1.recv();
        assert_eq!(packet_1.as_slice(), M1.as_bytes());
        println!("Got the first packet");

        let (packet_2,) = rx2.recv();
        assert_eq!(packet_2.as_slice(), M2.as_bytes());
        println!("Got the second packet");

        Ok(())
    }

    inner(num_threads).unwrap();

}

#[test]
fn talk_to_self_channel() {
    talk_to_self_channel_helper(1);
}
#[test]
fn talk_to_self_channel_parallel() {
    talk_to_self_channel_helper(4);
}

struct TestCallback {
    msg: &'static str,
    barrier: Arc<Barrier>,
}

impl MyFn<(::Packet,), ()> for TestCallback {

    fn call(&self, args: (::Packet,)) {
        let (packet,) = args;
        println!("got packet: {}", from_utf8(packet.as_slice()));
        println!("matching against: {}", from_utf8(self.msg.as_bytes()));
        assert_eq!(packet.as_slice(), self.msg.as_bytes());
        self.barrier.wait();
    }
}

fn talk_to_self_callback_helper(num_threads: uint) {

    fn mk_callback(barrier: Arc<Barrier>, msg: &'static str) -> ::Handler {
        box TestCallback { barrier: barrier, msg: msg }
    }

    fn inner(num_threads: uint) -> IoResult<()> {
        let barrier = Arc::new(Barrier::new(3));

        let (l1, a1) = try!(mk_listener(num_threads));
        let (l2, a2) = try!(mk_listener(num_threads));

        const M1: &'static str = "Hey Josh!";
        const M2: &'static str = "Hey Cody!";

        let interface1 = Interface::new(&l1, a2, mk_callback(barrier.clone(), M1));
        let interface2 = Interface::new(&l2, a1, mk_callback(barrier.clone(), M2));

        try!(::Interface::send(&interface1, String::from_str(M2).into_bytes()));
        try!(::Interface::send(&interface2, String::from_str(M1).into_bytes()));

        barrier.wait();

        Ok(())
    }

    inner(num_threads).unwrap();

}

#[test]
fn talk_to_self_callback() {
    talk_to_self_callback_helper(1);
}

#[test]
fn talk_to_self_callback_parallel() {
    talk_to_self_callback_helper(4);
}

#[test]
fn disable_then_cant_send() {

    fn inner() -> IoResult<()> {

        //let nop = box |&: _packet: Vec<u8>| { };

        let (l, a) = try!(mk_listener(1));
        let mut i = Interface::new(&l, a, box Nop);

        ::Interface::disable(&mut i);

        assert_eq!(::Interface::send(&i, Vec::new()).unwrap_err(),
                   // TODO: Report bug: shouldn't need prefix with `use super::*;` above
                   super::DISABLED_INTERFACE_ERROR);

        Ok(())
    }
    inner().unwrap();
}