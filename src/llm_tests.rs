

#[cfg(test)]
mod location_parsing_tests {
    use crate::llm::LlmClient;

    #[test]
    fn test_parse_location_valid_json() {
        let client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let json = r#"{"name":"Test Location","description":"A test description","image_prompt":"Test prompt","exits":{"north":null,"south":null,"east":null,"west":null},"items":[],"actors":[]}"#;
        let result = client.parse_location_json(json);
        assert!(result.is_ok(), "Failed to parse valid location: {:?}", result);
        let loc = result.unwrap();
        assert_eq!(loc.name, "Test Location");
    }

    #[test]
    fn test_parse_location_with_markdown() {
        let client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let json = r#"Here is the location:

```json
{"name":"Test Location","description":"A test","image_prompt":"Test","exits":{"north":null},"items":[],"actors":[]}
```"#;
        let result = client.parse_location_json(json);
        assert!(result.is_ok(), "Failed to parse location with markdown: {:?}", result);
    }

    #[test]
    fn test_parse_location_incomplete_json() {
        let client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let json = r#"{"name":"Test","description":"A test","image_prompt":"Test","exits":{"#;
        let result = client.parse_location_json(json);
        assert!(result.is_err(), "Should fail on incomplete JSON");
    }

    #[test]
    fn test_parse_location_extra_fields() {
        let client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let json = r#"{"name":"Test","description":"A test","image_prompt":"Test","exits":{"north":null},"items":[],"actors":[],"extra_field":"ignored"}"#;
        let result = client.parse_location_json(json);
        assert!(result.is_ok(), "Should ignore extra fields: {:?}", result);
    }

    #[test]
    fn test_is_complete_json_simple() {
        let client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        assert!(client.is_complete_json(r#"{"name":"test"}"#));
        assert!(!client.is_complete_json(r#"{"name":"test""#));
    }

    #[test]
    fn test_is_complete_json_nested() {
        let client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let json = r#"{"name":"test","nested":{"inner":"value","array":[1,2,3]}}"#;
        assert!(client.is_complete_json(json));
    }

    #[test]
    fn test_is_complete_json_with_strings() {
        let client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let json = r#"{"name":"test with \"quotes\"","desc":"valid"}"#;
        assert!(client.is_complete_json(json));
    }

    #[test]
    fn test_is_complete_json_with_backslash_in_string() {
        let client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let json = r#"{"name":"test","desc":"has a backslash: \\\""}"#;
        assert!(client.is_complete_json(json));
    }

    #[test]
    fn test_parse_location_with_whitespace() {
        let client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let json = r#"
        
        {"name":"Test","description":"A test"}
        
        "#;
        let result = client.parse_location_json(json);
        assert!(result.is_ok(), "Failed to parse location with whitespace: {:?}", result);
    }
}

#[cfg(test)]
mod world_update_parsing_tests {
    use crate::llm::LlmClient;

    #[test]
    fn test_parse_world_update_valid() {
        let client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let json = r#"{"narrative":"Test narrative","actions":[],"suggested_actions":["act1","act2"]}"#;
        let result = client.parse_content(json);
        assert!(result.is_ok(), "Failed to parse valid world update: {:?}", result);
    }

    #[test]
    fn test_parse_world_update_with_actions() {
        let client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let json = r#"{"narrative":"You move north","actions":["MoveTo(0,1)"],"suggested_actions":["go south"]}"#;
        let result = client.parse_content(json);
        assert!(result.is_ok(), "Failed to parse world update with actions: {:?}", result);
        let update = result.unwrap();
        assert_eq!(update.actions.len(), 1);
        assert_eq!(update.narrative, "You move north");
    }
}
