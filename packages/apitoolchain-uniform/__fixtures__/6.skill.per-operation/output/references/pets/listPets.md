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

## Example request

```shell
curl --request GET \
     --url https://api.example.com/v1/pets \
     --header 'accept: application/json'
```
