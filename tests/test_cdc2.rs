use fastcdc::v2020::StreamCDC;
fn main() {
    let source = std::io::Cursor::new(vec![0u8; 1024]);
    StreamCDC::new(source, 4194304, 8388608, 16777216);
}
