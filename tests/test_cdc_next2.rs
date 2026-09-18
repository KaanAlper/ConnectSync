use fastcdc::v2020::StreamCDC;
fn main() {
    let source = std::io::Cursor::new(vec![0u8; 1024]);
    let chunker = StreamCDC::new(source, 262144, 1048576, 4194304);
    for chunk in chunker {
        let c = chunk.unwrap();
        println!("len: {}, data_len: {}", c.length, c.data.len());
    }
}
