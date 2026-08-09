# BankPublicationEntity

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**bank_id** | **i32** |  |
**created_at** | **chrono::DateTime<chrono::FixedOffset>** |  |
**created_by** | Option<[**models::BankUserRef**](BankUserRef.md)> |  | [optional]
**id** | **i32** |  |
**public_id** | **String** |  |
**state** | **std::collections::HashMap<String, serde_json::Value>** |  |
**version** | **i32** |  |
**withdrawn_at** | Option<**chrono::DateTime<chrono::FixedOffset>**> |  | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
