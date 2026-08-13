# \SyncApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**ack_sync_cursor**](SyncApi.md#ack_sync_cursor) | **POST** /api/v1/sync/ack | Ack Sync Cursor
[**get_sync_snapshot**](SyncApi.md#get_sync_snapshot) | **GET** /api/v1/sync/snapshot | Get Sync Snapshot
[**pull_sync_changes**](SyncApi.md#pull_sync_changes) | **GET** /api/v1/sync/pull | Pull Sync Changes
[**push_sync_mutations**](SyncApi.md#push_sync_mutations) | **POST** /api/v1/sync/push | Push Sync Mutations



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
