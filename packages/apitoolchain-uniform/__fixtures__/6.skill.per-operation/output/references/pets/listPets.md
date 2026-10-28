<!-- Generated from the API reference. Do not edit by hand. -->

# `listPets` — List pets

`GET /pets`

Returns a page of pets, newest
first. Filters combine with AND.

Use `cursor` from a previous page to continue:
- pass `next_cursor` back as `cursor`
- keep the other filters as they were

## Query parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `limit` | integer | no | Maximum number of pets to return, at most 100. Default: `20`. Minimum: `1`. Maximum: `100`. |
| `status` | string | no | Only pets with this status. One of: `available`, `pending`, `sold`. |
| `tag` | array of string | no | Only pets carrying every one of these tags. |
| `cursor` | string | no | The `next_cursor` of the previous page. |
| `sort` | string | no | Deprecated. Sort order (use `order` instead). |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 200 | `application/json` | PetList | A page of pets. |
| 400 | `application/json` | Error | The filters are invalid. |

### 200 — `application/json`

| Name | Type | Description |
|---|---|---|
| `data` | array of Pet |  |
| `data[].id` | string | Unique identifier for the pet. |
| `data[].name` | string | The pet's name. |
| `data[].status` | string | Where the pet is in the sale. One of: `available`, `pending`, `sold`. |
| `data[].tags` | array of string |  |
| `data[].owner` | Owner | Who looks after the pet. |
| `has_more` | boolean | Whether another page follows. |
| `next_cursor` | string or null | Pass as `cursor` to fetch the next page. |

### 400 — `application/json`

| Name | Type | Description |
|---|---|---|
| `code` | string | A machine-readable error code. |
| `message` | string | A human-readable message. |

## Example request

```shell
curl --request GET \
     --url https://api.example.com/v1/pets \
     --header 'accept: application/json'
```
