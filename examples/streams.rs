use dioxus::prelude::*;
use dioxus_signals::use_signal;
use futures_util::{future, stream, Stream, StreamExt};
use std::time::Duration;

fn main() {
    dioxus_desktop::launch(app);
}

fn app() -> Element {
    let count = use_signal(|| 10);

    use_future(|_| async move {
        let mut stream = some_stream();

        while let Some(second) = stream.next().await {
            count.set(second);
        }
    });

    rsx! {
        h1 { "High-Five counter: {count}" }
    }
}

fn some_stream() -> std::pin::Pin<Box<dyn Stream<Item = i32>>> {
    Box::pin(
        stream::once(future::ready(0)).chain(stream::iter(1..).then(|second| async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            second
        })),
    )
}