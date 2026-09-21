use fastcdc::v2020::StreamCDC;
fn main() {
    let source = std::io::Cursor::new(vec![0u8; 1024]);
    let _chunker = StreamCDC::new(source, 262144, 1048576, 4194304);
}
