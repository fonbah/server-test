use std::sync::{atomic::{AtomicUsize, Ordering}, Arc};
use std::time::{Instant, Duration};

use tokio::net::TcpListener;
use tokio::{time, signal};
use tokio::io::Interest;
use hyper::{server::conn::Http, service::service_fn};
use hyper::{Error, Request, Response, Body};
use bytes::Bytes;
use http_body::Full;
use rand::Rng;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
    let mut listener = TcpListener::bind("127.0.0.1:3000").await?;
    println!("listening on 3000");

    // counters
    let clients_limit: u32 = 5;
    let mut clients_current: u32 = 0;

    let queue_counter = Arc::new(AtomicUsize::new(0));
    let _queue_counter = queue_counter.clone();

    let client_counter = Arc::new(AtomicUsize::new(0));
    let _client_counter = client_counter.clone();

    // graceful exit
    tokio::spawn(async move {
        if let Ok(_) = signal::ctrl_c().await {
            println!("\nctrl-c received!\nClients total served: {:?}", client_counter);
            println!("Clients total queue: {:?}", queue_counter);
            std::process::exit(1);
        }
    });

    loop {
        let ( mut stream, _ ) = listener.accept().await?;
        let now = Instant::now();

        // pending connections if limit reached
        if clients_current >= clients_limit {
            _queue_counter.fetch_add(1, Ordering::AcqRel);
            let mut interval = time::interval(time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                if clients_current < clients_limit ||
                    stream.ready(Interest::READABLE | Interest::WRITABLE).await.is_err() {
                    _queue_counter.fetch_sub(1, Ordering::AcqRel);
                    break;
                }
            }
            if stream.ready(Interest::READABLE | Interest::WRITABLE).await.is_err() {
                continue;
            }
        }

        clients_current +=1;
        _client_counter.fetch_add(1, Ordering::AcqRel);

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
        clients_current -=1;

        println!("Requests total {:?}", req_counter);
        println!("Time total {:?} ms", now.elapsed().as_millis());
    }
}
