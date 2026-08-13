# \SyncApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**ack_sync_cursor**](SyncApi.md#ack_sync_cursor) | **POST** /api/v1/sync/ack | Ack Sync Cursor
[**complete_sync_attachment_upload**](SyncApi.md#complete_sync_attachment_upload) | **POST** /api/v1/sync/attachments/uploads/{upload_id}/complete | Complete Sync Attachment Upload
[**download_sync_attachment**](SyncApi.md#download_sync_attachment) | **GET** /api/v1/sync/attachments/{attachment_id}/content | Download Sync Attachment
[**get_sync_attachment_upload**](SyncApi.md#get_sync_attachment_upload) | **GET** /api/v1/sync/attachments/uploads/{upload_id} | Get Sync Attachment Upload
[**get_sync_conflict**](SyncApi.md#get_sync_conflict) | **GET** /api/v1/sync/conflicts/{conflict_id} | Get Sync Conflict
[**get_sync_snapshot**](SyncApi.md#get_sync_snapshot) | **GET** /api/v1/sync/snapshot | Get Sync Snapshot
[**initiate_sync_attachment_upload**](SyncApi.md#initiate_sync_attachment_upload) | **POST** /api/v1/sync/attachments/uploads | Initiate Sync Attachment Upload
[**list_sync_entity_versions**](SyncApi.md#list_sync_entity_versions) | **GET** /api/v1/sync/entities/{entity_type}/{entity_id}/versions | List Sync Entity Versions
[**pull_sync_changes**](SyncApi.md#pull_sync_changes) | **GET** /api/v1/sync/pull | Pull Sync Changes
[**push_sync_mutations**](SyncApi.md#push_sync_mutations) | **POST** /api/v1/sync/push | Push Sync Mutations
[**put_sync_attachment_chunk**](SyncApi.md#put_sync_attachment_chunk) | **PUT** /api/v1/sync/attachments/uploads/{upload_id}/chunks/{ordinal} | Put Sync Attachment Chunk
[**resolve_sync_conflict**](SyncApi.md#resolve_sync_conflict) | **POST** /api/v1/sync/conflicts/{conflict_id}/resolve | Resolve Sync Conflict
[**restore_sync_entity_version**](SyncApi.md#restore_sync_entity_version) | **POST** /api/v1/sync/entities/{entity_type}/{entity_id}/versions/{version}/restore | Restore Sync Entity Version



## ack_sync_cursor

> models::EnvelopeSyncAckResponse ack_sync_cursor(sync_ack_request)
Ack Sync Cursor

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**sync_ack_request** | [**SyncAckRequest**](SyncAckRequest.md) |  | [required] |

### Return type

[**models::EnvelopeSyncAckResponse**](Envelope_SyncAckResponse_.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## complete_sync_attachment_upload

> models::EnvelopeAttachmentUploadStatus complete_sync_attachment_upload(upload_id, attachment_upload_complete_request)
Complete Sync Attachment Upload

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**upload_id** | **String** |  | [required] |
**attachment_upload_complete_request** | [**AttachmentUploadCompleteRequest**](AttachmentUploadCompleteRequest.md) |  | [required] |

### Return type

[**models::EnvelopeAttachmentUploadStatus**](Envelope_AttachmentUploadStatus_.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## download_sync_attachment

> std::path::PathBuf download_sync_attachment(attachment_id)
Download Sync Attachment

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**attachment_id** | **String** |  | [required] |

### Return type

[**std::path::PathBuf**](std::path::PathBuf.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/octet-stream, application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_sync_attachment_upload

> models::EnvelopeAttachmentUploadStatus get_sync_attachment_upload(upload_id)
Get Sync Attachment Upload

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**upload_id** | **String** |  | [required] |

### Return type

[**models::EnvelopeAttachmentUploadStatus**](Envelope_AttachmentUploadStatus_.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_sync_conflict

> models::EnvelopeSyncConflictRecord get_sync_conflict(conflict_id)
Get Sync Conflict

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**conflict_id** | **String** |  | [required] |

### Return type

[**models::EnvelopeSyncConflictRecord**](Envelope_SyncConflictRecord_.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_sync_snapshot

> models::EnvelopeSyncSnapshotResponse get_sync_snapshot(cursor, page_size)
Get Sync Snapshot

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**cursor** | Option<**String**> |  |  |
**page_size** | Option<**i32**> |  |  |[default to 100]

### Return type

[**models::EnvelopeSyncSnapshotResponse**](Envelope_SyncSnapshotResponse_.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## initiate_sync_attachment_upload

> models::EnvelopeAttachmentUploadStatus initiate_sync_attachment_upload(attachment_upload_initiate_request)
Initiate Sync Attachment Upload

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**attachment_upload_initiate_request** | [**AttachmentUploadInitiateRequest**](AttachmentUploadInitiateRequest.md) |  | [required] |

### Return type

[**models::EnvelopeAttachmentUploadStatus**](Envelope_AttachmentUploadStatus_.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## list_sync_entity_versions

> models::EnvelopeListSyncEntityVersionRecord list_sync_entity_versions(entity_type, entity_id)
List Sync Entity Versions

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**entity_type** | [**SyncEntityType**](SyncEntityType.md) |  | [required] |
**entity_id** | **String** |  | [required] |

### Return type

[**models::EnvelopeListSyncEntityVersionRecord**](Envelope_list_SyncEntityVersionRecord__.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## pull_sync_changes

> models::EnvelopeSyncPullResponse pull_sync_changes(cursor, page_size)
Pull Sync Changes

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**cursor** | Option<**String**> |  |  |
**page_size** | Option<**i32**> |  |  |[default to 100]

### Return type

[**models::EnvelopeSyncPullResponse**](Envelope_SyncPullResponse_.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## push_sync_mutations

> models::EnvelopeSyncPushResponse push_sync_mutations(sync_push_request)
Push Sync Mutations

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**sync_push_request** | [**SyncPushRequest**](SyncPushRequest.md) |  | [required] |

### Return type

[**models::EnvelopeSyncPushResponse**](Envelope_SyncPushResponse_.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## put_sync_attachment_chunk

> models::EnvelopeAttachmentChunkReceipt put_sync_attachment_chunk(upload_id, ordinal, x_chunk_sha256, body)
Put Sync Attachment Chunk

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**upload_id** | **String** |  | [required] |
**ordinal** | **i32** |  | [required] |
**x_chunk_sha256** | **String** |  | [required] |
**body** | **std::path::PathBuf** |  | [required] |

### Return type

[**models::EnvelopeAttachmentChunkReceipt**](Envelope_AttachmentChunkReceipt_.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/octet-stream
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## resolve_sync_conflict

> models::EnvelopeSyncConflictResolutionRecord resolve_sync_conflict(conflict_id, sync_conflict_resolution_request)
Resolve Sync Conflict

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**conflict_id** | **String** |  | [required] |
**sync_conflict_resolution_request** | [**SyncConflictResolutionRequest**](SyncConflictResolutionRequest.md) |  | [required] |

### Return type

[**models::EnvelopeSyncConflictResolutionRecord**](Envelope_SyncConflictResolutionRecord_.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## restore_sync_entity_version

> models::EnvelopeSyncVersionRestoreRecord restore_sync_entity_version(entity_type, entity_id, version, sync_version_restore_request)
Restore Sync Entity Version

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**entity_type** | [**SyncEntityType**](SyncEntityType.md) |  | [required] |
**entity_id** | **String** |  | [required] |
**version** | **i32** |  | [required] |
**sync_version_restore_request** | [**SyncVersionRestoreRequest**](SyncVersionRestoreRequest.md) |  | [required] |

### Return type

[**models::EnvelopeSyncVersionRestoreRecord**](Envelope_SyncVersionRestoreRecord_.md)

### Authorization

[bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)
