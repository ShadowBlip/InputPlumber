use packed_struct::{types::SizedInteger, PackedStruct};

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "64")]
pub struct PackedInputDataReport {
    
}

impl PackedInputDataReport {
    /// Return a new empty input data report
    pub fn new() -> Self {
        PackedInputDataReport {
           
        }
    }
}

impl Default for PackedInputDataReport {
    fn default() -> Self {
        Self::new()
    }
}