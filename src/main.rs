extern crate getopts;

use std::env;
use std::io::{self, BufReader, Read};
use std::io::prelude::*;
use std::net::{TcpStream, TcpListener};
use std::process::Command;
use std::str;
use std::thread;
use getopts::Options;

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
        client_sender(prog_opts);
    }

    else if prog_opts.listen {
        server_loop(prog_opts);
    }
}

fn client_sender(options: ProgOptions) {
    let stdin = io::stdin();
    let mut buffer: Vec<u8> = Vec::new(); // Build a buffer to send to the server

    {
        let mut handle = stdin.lock();
        let _ = handle.read_to_end(&mut buffer);
    }
    
    let mut stream = match TcpStream::connect((&options.target[..], options.port)) {
        Ok(s) => {
            s
        }
        Err(_) => {
            panic!("Could not connect to target.");
        }
    };

    loop {
        /* Transmit data to the server */
        let _ = stream.write_all(&buffer);
        
        /* Receive data from the server */
        let mut resp_arr = [0u8; READ_SIZE];
        match stream.read(&mut resp_arr) {
            Ok(resp_sz) => {
                print!("{}", str::from_utf8(&resp_arr[..resp_sz]).unwrap());
                let _ = io::stdout().flush();
            }
            Err(e) => {
                eprintln!("Something went wrong reading from server: {}", e);
                return;
            }
        };

        /* Read more data to send to server */

        {
            buffer.clear();
            let mut handle = stdin.lock();
            let _ = handle.read_to_end(&mut buffer);
        }
    }
}

fn run_command(command: &str) -> String {
    let (cmd_str, args) = match command.find(" ") {
        Some(n) => command.split_at(n),
        None => (command, "")
    };

    let output = match Command::new(cmd_str).args(args.split_whitespace()).output() {
        Ok(o) => { String::from_utf8(o.stdout).unwrap() }
        Err(_) => { String::from("Command failed to start\n") }
    };

    return output;
}

fn client_handler(mut stream: TcpStream, options: ProgOptions) {
    println!("Client connected.");
    let mut resp_str = String::new();

    if options.upload {
        let _ = stream.set_read_timeout(Some(std::time::Duration::new(10,0)));
        let mut read_buffer =[0u8; READ_SIZE];
        let mut file_buffer: Vec<u8> = Vec::new();

        loop {
            let read_sz = match stream.read(&mut read_buffer) {
                Ok(n) => n,
                Err(_) => { 0 }
            };

            let _ = println!("Read {} bytes", read_sz);

            if read_sz > 0 { file_buffer.append(&mut read_buffer.iter().cloned().collect()); }
            else { break; }
        }
            

        let mut f = std::fs::File::create(std::path::Path::new(options.upload_dest.as_str())).expect("Unable to create file");
        let _ = match f.write_all(&file_buffer) {
            Ok(_) => { stream.write(b"Wrote file\n") }
            Err(_) => { stream.write(b"Unable to write file\n") }
        };
    }

    /* If -e was requested at the command prompt we will execute the given command and present the results to the client */
    if options.execute.len() > 0 {
        let _ = stream.write(run_command(&options.execute).as_bytes());
    }

    /* If -c was requested at the command prompt we will present a command shell to the client and execute commands */
    if options.command {
        let _ = stream.set_read_timeout(None);
        loop {
            match stream.write(b"\n<RUNET:#> ") {
                Ok(_) => {
                    /* Read response from client */
                    {
                        let mut reader = BufReader::new(&stream);
                        let _ = match reader.read_line(&mut resp_str) {
                            Ok(n) => { n }
                            Err(e) => { eprintln!("Something bad happened: {}", e); return; }
                        };
                    }
                    let _ = stream.write(run_command(resp_str.trim()).as_bytes());
                    resp_str.clear();
                }
                Err(e) => { eprintln!("Something went wrong writing to the client: {}", e); return; }
            };
        }
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
                let opts = options.clone();
                thread::spawn(move || client_handler(stream, opts));
            }
            Err(_ ) => {}
        }
    }
}
