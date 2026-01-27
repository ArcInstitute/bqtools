pub type BoxedReader = Box<dyn std::io::Read + Send>;

#[allow(unused)]
pub type BoxedWriter = Box<dyn std::io::Write + Send>;
