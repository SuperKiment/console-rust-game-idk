//! Length-prefixed postcard framing over TCP, used by both client and server.

use std::io;

use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use serde::{Serialize, de::DeserializeOwned};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

const MAX_FRAME: usize = 1 << 20;

fn codec() -> LengthDelimitedCodec {
    LengthDelimitedCodec::builder().max_frame_length(MAX_FRAME).new_codec()
}

pub struct Reader(FramedRead<OwnedReadHalf, LengthDelimitedCodec>);
pub struct Writer(FramedWrite<OwnedWriteHalf, LengthDelimitedCodec>);

/// Splits a connected socket into a typed reader and writer.
pub fn split(stream: TcpStream) -> (Reader, Writer) {
    let _ = stream.set_nodelay(true);
    let (r, w) = stream.into_split();
    (Reader(FramedRead::new(r, codec())), Writer(FramedWrite::new(w, codec())))
}

impl Reader {
    /// Next message, `Ok(None)` when the peer closed the connection.
    pub async fn recv<T: DeserializeOwned>(&mut self) -> io::Result<Option<T>> {
        match self.0.next().await {
            None => Ok(None),
            Some(frame) => postcard::from_bytes(&frame?).map(Some).map_err(io::Error::other),
        }
    }
}

impl Writer {
    pub async fn send<T: Serialize>(&mut self, msg: &T) -> io::Result<()> {
        let bytes = postcard::to_stdvec(msg).map_err(io::Error::other)?;
        self.0.send(Bytes::from(bytes)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Biome;
    use crate::content::Tile;
    use crate::geo::{CHUNK_AREA, ChunkPos, Dir};
    use crate::protocol::{ChunkData, ClientMsg, ServerMsg};

    #[tokio::test]
    async fn messages_roundtrip_over_tcp() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let client = tokio::spawn(async move {
            let (mut r, mut w) = split(TcpStream::connect(addr).await.unwrap());
            w.send(&ClientMsg::Move(Dir::West)).await.unwrap();
            r.recv::<ServerMsg>().await.unwrap()
        });
        let (mut r, mut w) = split(listener.accept().await.unwrap().0);
        assert_eq!(r.recv::<ClientMsg>().await.unwrap(), Some(ClientMsg::Move(Dir::West)));
        let chunk = ServerMsg::Chunk(ChunkData {
            pos: ChunkPos::new(-3, 7),
            tiles: vec![Tile::Wall; CHUNK_AREA],
            biomes: vec![Biome::Plains; CHUNK_AREA],
        });
        w.send(&chunk).await.unwrap();
        assert_eq!(client.await.unwrap(), Some(chunk));
    }
}
