use serde_derive::{Deserialize, Serialize};
use serde_json::Result as SerdeResult;
use serde_json::{json, Result};
use tokio_socketcan::{CANFrame, CANSocket};

#[derive(Serialize, Deserialize, Debug)]
pub struct CanFrame {
    id: u32,
    data: [u8; 8],
    data_length: usize,
    is_remote: bool,
    is_error: bool,
}

impl CanFrame {
    pub fn new(
        id: u32,
        data: [u8; 8],
        data_length: usize,
        is_remote: bool,
        is_error: bool,
    ) -> Self {
        CanFrame {
            id,
            data,
            data_length,
            is_remote,
            is_error,
        }
    }

    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> String {
        serde_json::json!(self).to_string()
    }

    pub fn from_linux_frame(f: CANFrame) -> Self {
        let mut data = [0; 8];
        for i in 0..f.data().len() {
            data[i] = f.data()[i];
        }
        CanFrame {
            id: f.id(),
            data,
            data_length: f.data().len(),
            is_error: f.is_error(),
            is_remote: f.is_rtr(),
        }
    }
    pub fn to_linux_frame(&self) -> CANFrame {
        CANFrame::new(self.id, &self.data, self.is_remote, self.is_error).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use serde_json::Result;

    #[test]
    fn serialize_can_frame() -> Result<()> {
        let frame = r#"
            {
              "id": 123,
              "data": [123, 123, 23, 0, 0, 0, 0, 0],
              "data_length": 3,
              "is_remote": false,
              "is_error": false
            }
        "#;

        let f: CanFrame = CanFrame::from_json(frame)?;
        assert_eq!(f.data_length, 3);
        assert_eq!(f.is_remote, false);
        assert_eq!(f.is_remote, false);
        assert_eq!(f.data[2], 23);

        Ok(())
    }

    #[test]
    fn deserialize_can_frame() -> Result<()> {
        let frame = CanFrame::new(12, [3; 8], 4, true, false);

        let json = frame.to_json();
        let serialized: CanFrame = CanFrame::from_json(json.as_str())?;
        assert_eq!(serialized.data_length, 4);
        assert_eq!(serialized.is_remote, true);
        Ok(())
    }
}
