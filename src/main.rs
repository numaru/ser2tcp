use clap::{arg, command, Command};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::broadcast,
};

fn cli() -> Command {
    command!().args(&[
        arg!(--"serial-port" <SERIAL_PORT> "The device path to a serial port"),
        arg!(--baudrate <BAUDRATE> "The baudrate to connect at")
            .value_parser(clap::value_parser!(u32).range(9600..)),
        arg!(--"tcp-address" <TCP_ADDRESS> "The address to bind the tcp server on"),
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
    let tcp_address = matches.get_one::<String>("tcp-address").unwrap().clone();

    let listener = TcpListener::bind(tcp_address).await?;
    let (tx_base, _) = broadcast::channel::<(Vec<u8>, usize)>(16);

    let tx = tx_base.clone();
    let mut rx = tx.subscribe();

    tokio::spawn(async move {
        let self_uid = generate_uid();
        let mut serial_port = serialport::new(serial_port, baudrate).open().unwrap();
        tokio::select! {
            result = async {
                let mut buffer: Vec<u8> = vec![0; 1024];
                serial_port.read(&mut buffer)?;
                io::Result::<Vec<u8>>::Ok(buffer)
             } => {
                let buffer = result.unwrap();
                tx.send((buffer, self_uid)).unwrap();
            }
            result = rx.recv() => {
                let (msg, rx_uid) = result.unwrap();
                if rx_uid != self_uid {
                    serial_port.write(&msg).unwrap();
                }
            }
        }
    });

    loop {
        let (mut socket, _) = listener.accept().await?;

        let tx = tx_base.clone();
        let mut rx = tx.subscribe();

        tokio::spawn(async move {
            let self_uid = generate_uid();
            let (mut reader, mut writer) = socket.split();
            loop {
                tokio::select! {
                    result = async {
                        let mut buffer: Vec<u8> = vec![0; 1024];
                        reader.read(&mut buffer).await?;
                        io::Result::<Vec<u8>>::Ok(buffer)
                     } => {
                        let buffer = result.unwrap();
                        tx.send((buffer, self_uid)).unwrap();
                    }
                    result = rx.recv() => {
                        let (msg, rx_uid) = result.unwrap();
                        if rx_uid != self_uid {
                            writer.write(&msg).await.unwrap();
                        }
                    }
                }
            }
        });
    }
}
