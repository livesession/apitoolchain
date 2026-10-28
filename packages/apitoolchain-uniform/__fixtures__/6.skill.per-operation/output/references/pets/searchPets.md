<!-- Generated from the API reference. Do not edit by hand. -->

# `searchPets` — Search pets

`POST /pets/search`

Searches pets by name. A read-only POST — the query is too rich for a URL.

## Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `query` | string | yes | Text to match against pet names. Example: `rex`. |
| `filters` | object | no | Narrow the matches; every filter must hold: - status - tags |
| `filters.status` | string | no | One of: `available`, `pending`, `sold`. |
| `filters.tags` | array of string | no |  |
| `limit` | integer | no | Maximum number of matches. Default: `100`. |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 200 | `application/json` | PetList | The matching pets. |

## Example request

```shell
curl --request POST \
     --url https://api.example.com/v1/pets/search \
     --header 'accept: application/json' \
     --header 'content-type: application/json' \
     --data '
{
  "query": "rex",
  "filters": {
    "status": "available",
    "tags": [
      "string"
    ]
  },
  "limit": 100
}
'
```
