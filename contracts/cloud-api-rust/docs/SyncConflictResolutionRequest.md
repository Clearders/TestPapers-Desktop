# SyncConflictResolutionRequest

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**action** | [**models::SyncResolutionAction**](SyncResolutionAction.md) |  |
**current_content_hash** | **String** |  |
**current_version** | **i32** |  |
**new_entity_id** | Option<**String**> |  | [optional]
**operation_id** | **String** |  |
**payload** | Option<**std::collections::HashMap<String, serde_json::Value>**> |  | [optional]
**protocol_version** | **ProtocolVersion** |  (enum: 1) |
**undoes_resolution_id** | Option<**String**> |  | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
