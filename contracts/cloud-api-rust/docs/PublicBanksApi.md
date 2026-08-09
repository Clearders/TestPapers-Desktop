# \PublicBanksApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_public_bank_snapshot**](PublicBanksApi.md#get_public_bank_snapshot) | **GET** /api/v1/public/banks/{bank_public_id} | Get Public Bank Snapshot
[**list_public_bank_snapshots**](PublicBanksApi.md#list_public_bank_snapshots) | **GET** /api/v1/public/banks | List Public Bank Snapshots



## get_public_bank_snapshot

> models::EnvelopePublicBankDetail get_public_bank_snapshot(bank_public_id)
Get Public Bank Snapshot

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bank_public_id** | **String** |  | [required] |

### Return type

[**models::EnvelopePublicBankDetail**](Envelope_PublicBankDetail_.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## list_public_bank_snapshots

> models::EnvelopeListPublicBankSummary list_public_bank_snapshots(q)
List Public Bank Snapshots

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**q** | Option<**String**> |  |  |

### Return type

[**models::EnvelopeListPublicBankSummary**](Envelope_list_PublicBankSummary__.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)
