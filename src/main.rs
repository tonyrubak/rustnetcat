extern crate getopts;

use std::env;
use std::io::{self, Read};
use std::io::prelude::*;
use std::net::{TcpStream, TcpListener};
use std::str;
use std::thread;
use getopts::Options;

const BUFFER_SIZE: usize = 8192;
const READ_SIZE: usize = 4096;

#[derive(Clone)]
struct ProgOptions {
    listen: bool,
    command: bool,
    upload: bool,
    execute: String,
    target: String,
    upload_dest: String,
    port: u16,
}

fn usage() {
    println!("Rust Net Tool");
    println!("Usage runet -t target_host -p port");
    println!("-l --listen              - listen on [host]:[port] for incoming connections");
    println!("-e --execute=file_to_run - execute the given file upon receiving a connection");
    println!("-c --command             - initialize a command shell");
    println!("-u --upload=destination  - upon receiving connection upload a file and write to [destination]");
    std::process::exit(0);
}

fn main() {
    let args: Vec<_> = env::args().collect();

    let mut prog_opts = ProgOptions {
        listen: false,
        command: false,
        upload: false,
        execute: String::new(),
        target: String::new(),
        upload_dest: String::new(),
        port: 0,
    };

    /* No command line arguments were passed; print usage and quit */
    if args.len() == 1 {
        usage();
    }

    /* Parse command-line arguments */
    let mut opts = Options::new();
    opts.reqopt("t", "target", "", "TARGET");
    opts.reqopt("p", "port", "", "PORT");
    opts.optflag("c", "command", "initialize a command shell");
    opts.optflag("l", "listen", "listen on [host]:[port] for incoming connections");
    opts.optflag("h", "help", "");
    opts.optopt("e", "execute", "execute the given file upon receiving a connection", "FILE");
    opts.optopt("u", "upload", "upon receiving connection upload a file and write to [destination]", "DESTINATION");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m }
        Err(_) => { usage(); return; }
    };

    if matches.opt_present("h") {
        usage();
    }
    if matches.opt_present("c") {
        prog_opts.command = true;
    }
    if matches.opt_present("l") {
        prog_opts.listen = true;
    }
    if matches.opt_present("e") {
        prog_opts.execute = matches.opt_str("e").unwrap();
    }
    if matches.opt_present("u") {
        prog_opts.upload = true;
        prog_opts.upload_dest = matches.opt_str("u").unwrap();
    }
    prog_opts.target = matches.opt_str("t").unwrap();
    prog_opts.port = match matches.opt_str("p").unwrap().parse::<u16>() {
        Err(_) => { usage(); return; }
        Ok(i) => { i }
    };

    /* If we are not listening then we will read from stdin and write to the target */
    if !prog_opts.listen && prog_opts.target.len() > 0 && prog_opts.port > 0 {
        let mut buffer = Vec::new();
        let stdin = io::stdin();
        {
            let mut handle = stdin.lock();
            let _ = handle.read_to_end(&mut buffer);
        }
        client_sender(buffer, prog_opts);
    }

    else if prog_opts.listen {
        server_loop(prog_opts);
    }
}

fn vec_to_arr(vector: Vec<u8>, arr: &mut [u8;BUFFER_SIZE]) -> usize {
    let size = if vector.len() > BUFFER_SIZE {
        BUFFER_SIZE
    } else { vector.len() };
    for i in 0..(size - 1) {
        arr[i] = vector[i];
    }
    return size;
}

fn client_sender(buffer: Vec<u8>, options: ProgOptions) {
    let mut mbuffer = buffer.clone();
    let mut stream = match TcpStream::connect((&options.target[..], options.port)) {
        Ok(s) => {
            s
        }
        Err(_) => {
            panic!("Could not connect to target.");
        }
    };

    let mut sent: usize = 0;

    loop {
        /* Transmit data to the server */
        while sent < mbuffer.len() {
            let mut arr = [0u8; BUFFER_SIZE];
            let _ = vec_to_arr(mbuffer[sent..].to_vec(), &mut arr);
            
            let sz = match stream.write(&arr) {
                Ok(n) => n,
                Err(_) => { panic!("Failed to send data.") }
            };
            
            sent += sz;
        }
        
        /* Receive data from the server */
        let mut resp_arr = [0u8; READ_SIZE];
        let response = stream.read(&mut resp_arr);
        print!("{}", str::from_utf8(&resp_arr[..response.unwrap()]).unwrap());
        io::stdout().flush();

        /* Read more data to send to server */
        let stdin = io::stdin();
        {
            let mut handle = stdin.lock();
            let _ = handle.read_to_end(&mut mbuffer);
        }
        sent = 0;
    }
}

fn server_loop(options: ProgOptions) {
    let target = if options.target.len() == 0 {
        "0.0.0.0"
    } else { &options.target[..] };

    let listener = TcpListener::bind((target, options.port)).unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {

            }
            Err(_ ) => {}
        }
    }
}
