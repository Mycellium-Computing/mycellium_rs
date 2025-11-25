/// Returns the request topic name and the response topic name for a given functionality name.
pub fn get_topic_names(functionality_name: &str) -> (String, String) {
    let request_topic = format!("request.{}", functionality_name);
    let response_topic = format!("response.{}", functionality_name);
    (request_topic, response_topic)
}

/// Returns the request topic type name and the response topic type name for a given input and output type.
pub fn get_request_response_topic_type_names(
    input_type: String,
    output_type: String,
) -> (String, String) {
    let request_type_name = format!("ProviderExchange<{}>", input_type);
    let response_type_name = format!("ProviderExchange<{}>", output_type);
    (request_type_name, response_type_name)
}

/// Returns the empty message type name.
pub fn get_empty_message_type_name() -> String {
    "EmptyMessage".to_string()
}
