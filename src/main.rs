use clap::{arg, command, Command};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::{
    io::{
        self, AsyncReadExt, AsyncWriteExt,
        ErrorKind::{BrokenPipe, TimedOut},
    },
    net::TcpListener,
    sync::broadcast,
};
use tokio_serial::SerialPortBuilderExt;

fn cli() -> Command {
    command!().args(&[
        arg!(--"serial-port" <SERIAL_PORT> "The device path to a serial port").required(true),
        arg!(--baudrate <BAUDRATE> "The baudrate to connect at")
            .value_parser(clap::value_parser!(u32).range(9600..))
            .required(true),
        arg!(--"tcp-port" <TCP_PORT> "The port to bind the tcp server on")
            .value_parser(clap::value_parser!(u32))
            .required(true),
    ])
}

fn generate_uid() -> usize {
    static UID_COUNTER: AtomicUsize = AtomicUsize::new(1);
    UID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let matches = cli().get_matches();

    let serial_port = matches.get_one::<String>("serial-port").unwrap().clone();
    let baudrate = *matches.get_one::<u32>("baudrate").unwrap();
    let tcp_port = matches.get_one::<u32>("tcp-port").unwrap();

    let listener = TcpListener::bind(format!("0.0.0.0:{}", tcp_port)).await?;
    let (base_tx, _) = broadcast::channel::<(Vec<u8>, usize)>(256);

    let tx = base_tx.clone();
    let mut rx = tx.subscribe();

    tokio::spawn(async move {
        let self_uid = generate_uid();
        let mut serial_port = tokio_serial::new(serial_port, baudrate)
            .open_native_async()
            .unwrap();
        let mut buffer = [0; 1024];
        loop {
            tokio::select! {
                result = serial_port.read(&mut buffer) => {
                    match result {
                        Ok(n) if n == 0 => {
                            // TODO: close the serial_port
                            println!("close the serial_port");
                        },
                        Ok(n) => {
                            tx.send((buffer[0..n].to_vec(), self_uid)).unwrap();
                        },
                        Err(ref e) if e.kind() == TimedOut => (),
                        Err(ref e) if e.kind() == BrokenPipe => (),
                        Err(e) => {
                            println!("{:?}", e);
                        },
                    };
                }
                result = rx.recv() => {
                    let (msg, rx_uid) = result.unwrap();
                    if rx_uid != self_uid {
                        serial_port.write(&msg).await.unwrap();
                        serial_port.flush().await.unwrap();
                    }
                }
            }
        }
    });

    loop {
        let (mut socket, _) = listener.accept().await?;

        let tx = base_tx.clone();
        let mut rx = tx.subscribe();

        let self_uid = generate_uid();

        tokio::spawn(async move {
            println!("uid: {}", self_uid);
            let (mut reader, mut writer) = socket.split();
            let mut buffer = [0; 1024];
            loop {
                tokio::select! {
                    result = reader.read(&mut buffer) => {
                        match result {
                            Ok(n) if n == 0 => {
                                // TODO: close the socket
                                println!("close the socket");
                            },
                            Ok(n) => {
                                tx.send((buffer[0..n].to_vec(), self_uid)).unwrap();
                            },
                            Err(e) => {
                                println!("{:?}", e);
                            },
                        };
                    }
                    result = rx.recv() => {
                        let (msg, rx_uid) = result.unwrap();
                        if rx_uid != self_uid {
                            writer.write(&msg).await.unwrap();
                            writer.flush().await.unwrap();
                        }
                    }
                }
            }
        });
    }
}
