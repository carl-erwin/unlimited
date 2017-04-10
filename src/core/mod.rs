//
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;

//
use self::futures::{Future, Stream};
use self::tokio_io::{io, AsyncRead};
use self::tokio_core::net::TcpListener;
use self::tokio_core::reactor::Core;


//
pub mod editor;
pub mod config;
pub mod screen;
pub mod codepointinfo;
pub mod buffer;
pub mod byte_buffer;
pub mod event;
pub mod view;
pub mod mark;
pub mod text_codec;




// start core thread
pub fn start() {

    // Create the event loop that will drive this server
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    // Bind the server's socket
    let addr = "127.0.0.1:5000".parse().unwrap();
    let tcp = TcpListener::bind(&addr, &handle).unwrap();

    // Iterate incoming connections
    let server = tcp.incoming()
        .for_each(|(tcp, _)| {
            // Split up the read and write halves
            let (reader, writer) = tcp.split();

            // Future of the copy
            let bytes_copied = io::copy(reader, writer);

            // ... after which we'll print what happened
            let handle_conn = bytes_copied
                .map(|(n, _, _)| println!("wrote {} bytes", n))
                .map_err(|err| println!("IO error {:?}", err));

            // Spawn the future as a concurrent task
            handle.spawn(handle_conn);

            Ok(())
        });

    // Spin up the server on the event loop
    core.run(server).unwrap();
}


// TODO: return a status , ex waiting for job to finsh etc
pub fn stop() {}
