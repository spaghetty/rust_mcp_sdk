#[cfg(test)]
mod tool_arguments_derive_tests {
    use mcp_sdk::ToolArguments; // The re-exported derive macro from mcp_sdk crate
    use mcp_sdk::ToolArgumentsDescriptor; // The trait from mcp_sdk crate
    use serde_json::{json, Value};

    // 1. Basic Struct
    #[derive(ToolArguments)]
    struct BasicArgs {
        name: String,
        age: i32,
    }

    #[test]
    fn test_basic_args() {
        let schema = BasicArgs::mcp_input_schema();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["name"]["type"], "string");
        assert_eq!(schema["properties"]["age"]["type"], "integer");

        let required_fields: Vec<String> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert!(required_fields.contains(&"name".to_string()));
        assert!(required_fields.contains(&"age".to_string()));
        assert_eq!(required_fields.len(), 2);
    }

    // 2. Struct with Option<T>
    #[derive(ToolArguments)]
    struct OptionalArgs {
        id: String,
        description: Option<String>,
    }

    #[test]
    fn test_optional_args() {
        let schema = OptionalArgs::mcp_input_schema();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["id"]["type"], "string");
        assert_eq!(schema["properties"]["description"]["type"], "string"); // Option<String> maps to "string" for schema type

        let required_fields: Vec<String> = schema["required"]
            .as_array()
            .map_or_else(Vec::new, |arr| arr.iter().map(|v| v.as_str().unwrap().to_string()).collect());

        assert!(required_fields.contains(&"id".to_string()));
        assert!(!required_fields.contains(&"description".to_string()));
        assert_eq!(required_fields.len(), 1);
    }

    // 3. Struct with Vec<T>
    #[derive(ToolArguments)]
    struct ListArgs {
        items: Vec<String>,
        counts: Vec<i32>,
    }

    #[test]
    fn test_list_args() {
        let schema = ListArgs::mcp_input_schema();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["items"]["type"], "array");
        assert_eq!(schema["properties"]["items"]["items"]["type"], "string");
        assert_eq!(schema["properties"]["counts"]["type"], "array");
        assert_eq!(schema["properties"]["counts"]["items"]["type"], "integer");

        let required_fields: Vec<String> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert!(required_fields.contains(&"items".to_string()));
        assert!(required_fields.contains(&"counts".to_string()));
        assert_eq!(required_fields.len(), 2);
    }

    // 4. Struct with #[tool_arg(desc = "...")]
    #[derive(ToolArguments)]
    struct DescArgs {
        #[tool_arg(desc = "The unique identifier.")]
        id: String,
    }

    #[test]
    fn test_desc_args() {
        let schema = DescArgs::mcp_input_schema();
        assert_eq!(schema["properties"]["id"]["description"], "The unique identifier.");
        assert_eq!(schema["properties"]["id"]["type"], "string");
    }

    // 5. Struct with #[tool_arg(rename = "...")]
    #[derive(ToolArguments)]
    struct RenameArgs {
        #[tool_arg(rename = "userIdentifier")]
        user_id: String,
    }

    #[test]
    fn test_rename_args() {
        let schema = RenameArgs::mcp_input_schema();
        assert!(schema["properties"].get("user_id").is_none());
        assert_eq!(schema["properties"]["userIdentifier"]["type"], "string");
        let required_fields: Vec<String> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert!(required_fields.contains(&"userIdentifier".to_string()));
    }

    // 6. Struct with #[tool_arg(skip)]
    #[derive(ToolArguments)]
    struct SkipArgs {
        visible: String,
        #[tool_arg(skip)]
        internal_data: i32,
    }

    #[test]
    fn test_skip_args() {
        let schema = SkipArgs::mcp_input_schema();
        assert!(schema["properties"].get("internal_data").is_none());
        assert_eq!(schema["properties"]["visible"]["type"], "string");
        let required_fields: Vec<String> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert!(required_fields.contains(&"visible".to_string()));
        assert_eq!(required_fields.len(), 1);
    }

    // 7. Struct with #[tool_arg(required = ...)] overrides
    #[derive(ToolArguments)]
    struct RequiredOverrideArgs {
        #[tool_arg(required = false)] // Make non-Option not required
        name: String,
        #[tool_arg(required = true)]  // Make Option required
        description: Option<String>,
        // Implicitly required Option
        #[tool_arg(required = true)]
        code: Option<i32>,
        // Implicitly not required non-Option
        #[tool_arg(required = false)]
        status: String,
    }

    #[test]
    fn test_required_override_args() {
        let schema = RequiredOverrideArgs::mcp_input_schema();
        assert_eq!(schema["properties"]["name"]["type"], "string");
        assert_eq!(schema["properties"]["description"]["type"], "string");
        assert_eq!(schema["properties"]["code"]["type"], "integer");
        assert_eq!(schema["properties"]["status"]["type"], "string");

        let required_fields: Vec<String> = schema["required"]
            .as_array()
            .map_or_else(Vec::new, |arr| arr.iter().map(|v| v.as_str().unwrap().to_string()).collect());

        assert!(!required_fields.contains(&"name".to_string()), "name should not be required");
        assert!(required_fields.contains(&"description".to_string()), "description should be required");
        assert!(required_fields.contains(&"code".to_string()), "code should be required");
        assert!(!required_fields.contains(&"status".to_string()), "status should not be required");
        assert_eq!(required_fields.len(), 2, "Expected 2 required fields: description, code. Got: {:?}", required_fields);
    }

    // 8. Nested Struct
    #[derive(ToolArguments, Debug, Clone)] // Added Debug, Clone for potential use in other tests later
    struct NestedInner {
        detail: String,
        #[tool_arg(desc = "An optional detail code")]
        detail_code: Option<i32>,
    }

    #[derive(ToolArguments)]
    struct NestedOuter {
        id: i32,
        inner_data: NestedInner,
        optional_inner: Option<NestedInner>,
        #[tool_arg(desc = "A list of inner details")]
        inner_list: Vec<NestedInner>,
    }

    #[test]
    fn test_nested_struct_args() {
        let inner_schema_expected = json!({
            "type": "object",
            "properties": {
                "detail": {"type": "string"},
                "detail_code": {"type": "integer", "description": "An optional detail code"}
            },
            "required": ["detail"]
        });

        let inner_schema_actual = NestedInner::mcp_input_schema();
        assert_eq!(inner_schema_actual, inner_schema_expected, "NestedInner schema mismatch");

        let outer_schema = NestedOuter::mcp_input_schema();
        assert_eq!(outer_schema["type"], "object");
        assert_eq!(outer_schema["properties"]["id"]["type"], "integer");

        // Check inner_data (required)
        assert_eq!(outer_schema["properties"]["inner_data"], inner_schema_expected, "inner_data schema mismatch");

        // Check optional_inner (not required, but schema is the same)
        assert_eq!(outer_schema["properties"]["optional_inner"], inner_schema_expected, "optional_inner schema mismatch");

        // Check inner_list (required, array of inner_schema)
        assert_eq!(outer_schema["properties"]["inner_list"]["type"], "array");
        assert_eq!(outer_schema["properties"]["inner_list"]["items"], inner_schema_expected, "inner_list items schema mismatch");
        assert_eq!(outer_schema["properties"]["inner_list"]["description"], "A list of inner details");


        let required_fields: Vec<String> = outer_schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();

        assert!(required_fields.contains(&"id".to_string()));
        assert!(required_fields.contains(&"inner_data".to_string()));
        assert!(!required_fields.contains(&"optional_inner".to_string()));
        assert!(required_fields.contains(&"inner_list".to_string()));
        assert_eq!(required_fields.len(), 3, "Outer required fields mismatch. Got: {:?}", required_fields);
    }
}
