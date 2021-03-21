use serde_derive::{Deserialize, Serialize};
use serde_json::Result;
use tokio_socketcan::CANFrame;

#[derive(Serialize, Deserialize, Debug)]
pub struct CanFrame {
    id: u32,
    data: Vec<u8>,
    is_remote: bool,
    is_error: bool,
}

impl CanFrame {
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> String {
        serde_json::json!(self).to_string()
    }

    pub fn from_linux_frame(f: CANFrame) -> Self {
        let mut data = vec![];
        for i in 0..f.data().len() {
            data.push(f.data()[i]);
        }
        CanFrame {
            id: f.id(),
            data,
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
    use serde_json::Result;

    use super::*;

    #[test]
    fn serialize_can_frame() -> Result<()> {
        let frame = r#"
            {
              "id": 123,
              "data": [123, 123, 23],
              "is_remote": false,
              "is_error": false
            }
        "#;

        let f: CanFrame = CanFrame::from_json(frame)?;
        assert_eq!(f.data.len(), 3);
        assert_eq!(f.is_remote, false);
        assert_eq!(f.is_remote, false);
        assert_eq!(f.data[2], 23);

        Ok(())
    }

    #[test]
    fn deserialize_can_frame() -> Result<()> {
        let frame = CanFrame {
            id: 12,
            data: vec![3; 5],
            is_remote: true,
            is_error: false,
        };

        let json = frame.to_json();
        let serialized: CanFrame = CanFrame::from_json(json.as_str())?;
        assert_eq!(serialized.data.len(), 5);
        assert_eq!(serialized.is_remote, true);
        Ok(())
    }
}
