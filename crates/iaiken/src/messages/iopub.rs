use crate::messages::crypto::sign_message;
use crate::messages::{JupyterMessage, MessageHeader};

fn build_pub(
    header: MessageHeader,
    parent_header: Option<crate::messages::MessageHeader>,
    metadata: serde_json::Value,
    content: serde_json::Value,
    key: &str,
    scheme: &str,
) -> anyhow::Result<Vec<bytes::Bytes>> {
    let h = serde_json::to_vec(&header)?;
    let p = serde_json::to_vec(&parent_header)?;
    let m = serde_json::to_vec(&metadata)?;
    let c = serde_json::to_vec(&content)?;
    let sig = sign_message(key, scheme, &h, &p, &m, &c).into_bytes();
    Ok(vec![b"<IDS|MSG>".to_vec(), sig, h, p, m, c]
        .into_iter()
        .map(Into::into)
        .collect())
}

impl JupyterMessage<serde_json::Value> {
    // DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#kernel-status
    pub fn to_iopub_status(
        &self,
        key: &str,
        scheme: &str,
        state: &str,
    ) -> anyhow::Result<Vec<bytes::Bytes>> {
        let header = MessageHeader::new(self.header.session.clone(), "status".to_string());
        let parent = Some(self.header.clone());
        let metadata = serde_json::Value::Object(serde_json::Map::new());
        let content = serde_json::json!({ "execution_state": state });
        build_pub(header, parent, metadata, content, key, scheme)
    }

    // DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#code-inputs
    pub fn to_iopub_execute_input(
        &self,
        key: &str,
        scheme: &str,
        code: &str,
        execution_count: u32,
    ) -> anyhow::Result<Vec<bytes::Bytes>> {
        let header = MessageHeader::new(self.header.session.clone(), "execute_input".to_string());
        let parent = Some(self.header.clone());
        let metadata = serde_json::Value::Object(serde_json::Map::new());
        let content = serde_json::json!({ "code": code, "execution_count": execution_count });
        build_pub(header, parent, metadata, content, key, scheme)
    }

    // DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#streams-stdout-stderr-etc
    pub fn to_iopub_stream(
        &self,
        key: &str,
        scheme: &str,
        name: &str,
        text: &str,
    ) -> anyhow::Result<Vec<bytes::Bytes>> {
        let header = MessageHeader::new(self.header.session.clone(), "stream".to_string());
        let parent = Some(self.header.clone());
        let metadata = serde_json::Value::Object(serde_json::Map::new());
        let content = serde_json::json!({ "name": name, "text": text });
        build_pub(header, parent, metadata, content, key, scheme)
    }

    // DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#id7
    pub fn to_iopub_execute_result(
        &self,
        key: &str,
        scheme: &str,
        execution_count: u32,
        data: serde_json::Value,
        metadata: serde_json::Value,
    ) -> anyhow::Result<Vec<bytes::Bytes>> {
        let header = MessageHeader::new(self.header.session.clone(), "execute_result".to_string());
        let parent = Some(self.header.clone());
        let content = serde_json::json!({
            "execution_count": execution_count,
            "data": data,
            "metadata": metadata
        });
        build_pub(
            header,
            parent,
            serde_json::Value::Object(serde_json::Map::new()),
            content,
            key,
            scheme,
        )
    }

    // DOCS: https://jupyter-client.readthedocs.io/en/latest/messaging.html#request-reply
    pub fn to_iopub_error(
        &self,
        key: &str,
        scheme: &str,
        ename: &str,
        evalue: &str,
        traceback: &[String],
    ) -> anyhow::Result<Vec<bytes::Bytes>> {
        let header = MessageHeader::new(self.header.session.clone(), "error".to_string());
        let parent = Some(self.header.clone());
        let metadata = serde_json::Value::Object(serde_json::Map::new());
        let content =
            serde_json::json!({ "ename": ename, "evalue": evalue, "traceback": traceback });
        build_pub(header, parent, metadata, content, key, scheme)
    }
}
