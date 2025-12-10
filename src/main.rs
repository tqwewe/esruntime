use umadb_client::UmaDBClient;
use umadb_dcb::{DCBEvent, DCBEventStoreSync};

fn main() -> anyhow::Result<()> {
    let client = UmaDBClient::new("http://0.0.0.0:50051".to_string()).connect()?;

    client.append(
        vec![DCBEvent {
            event_type: "JoinedDCBGang".to_string(),
            tags: vec!["ari".to_string()],
            data: b"woohoo!".to_vec(),
            uuid: None,
        }],
        None,
    )?;

    let (events, head) = client
        .read(None, None, false, None, false)?
        .collect_with_head()?;
    dbg!(events, head);

    Ok(())
}
