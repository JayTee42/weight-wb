use super::Printer;

use std::time::Duration;

const IO_TIMEOUT: Duration = Duration::from_millis(500);

impl Printer {
    pub(super) fn read(&self, data: &mut [u8]) -> Result<usize, rusb::Error> {
        self.handle.read_bulk(self.in_addr, data, IO_TIMEOUT)
    }

    pub(super) fn write(&self, data: &[u8]) -> Result<(), rusb::Error> {
        let written_bytes = self.handle.write_bulk(self.out_addr, data, IO_TIMEOUT)?;

        // Can this happen at all ... ? Never seen it ...
        if written_bytes != data.len() {
            eprintln!(
                "Number of written bytes does not equal the input slice (expected {}, got {}).",
                data.len(),
                written_bytes
            );
        }

        Ok(())
    }
}
