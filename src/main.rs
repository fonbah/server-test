use std::sync::{atomic::{AtomicUsize, Ordering}, Arc};
use std::time::{Instant, Duration};

use tokio::net::{TcpListener, TcpStream};
use tokio::{time, signal};
use tokio::sync::mpsc;
use hyper::{server::conn::Http, service::service_fn};
use hyper::{Error, Request, Response, Body};
use bytes::Bytes;
use http_body::Full;
use rand::Rng;

async fn request_service(stream: TcpStream)->Result<(), Box<dyn std::error::Error + 'static>> {
    let now = Instant::now();

    let req_counter = Arc::new(AtomicUsize::new(0));
    let _req_counter = req_counter.clone();

    let service = service_fn(move |_req: Request<Body>| {
        let count = _req_counter.fetch_add(1, Ordering::AcqRel);

        async move {
            let tm_rand = rand::thread_rng().gen_range(100..500);
            time::sleep(Duration::from_millis(tm_rand)).await;
            Ok::<_, Error>(Response::new(Full::new(Bytes::from(format!(
                "Request #{}",
                count
            )))))
        }
    });

    if let Err(http_err) = Http::new()
        .http2_only(true)
        .serve_connection(stream, service)
        .await {
        println!("Error while serving HTTP connection: {}", http_err);
    }

    println!("Requests total {:?}", req_counter);
    println!("Time total {:?} ms", now.elapsed().as_millis());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
    let mut listener = TcpListener::bind("127.0.0.1:3000").await?;
    println!("listening on 3000");

    let clients_limit = 5;

    // counters
    let client_counter = Arc::new(AtomicUsize::new(0));
    let _client_counter = client_counter.clone();

    let (tx, mut rx) = mpsc::channel::<TcpStream>(clients_limit);
    let _tx = tx.clone();

    // graceful exit
    tokio::spawn(async move {
        if let Ok(_) = signal::ctrl_c().await {
            println!("\nctrl-c received!\nClients total served: {:?}", client_counter);
            println!("Clients total queue: {:?}", clients_limit - _tx.capacity());
            std::process::exit(1);
        }
    });

    // connection service
    tokio::spawn(async move {
        while let Some(stream) = rx.recv().await {
            _client_counter.fetch_add(1, Ordering::AcqRel);
            request_service(stream).await;
        };
    });

    loop {
        let ( mut stream, _ ) = listener.accept().await?;
        let sender = tx.clone();
        sender.send(stream).await;
    }
}
