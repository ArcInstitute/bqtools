pub type BoxedReader = Box<dyn std::io::Read + Send>;
pub type BoxedWriter = Box<dyn std::io::Write + Send>;
