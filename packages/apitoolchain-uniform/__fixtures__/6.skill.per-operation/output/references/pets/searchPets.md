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
| `limit` | integer | no | Maximum number of matches, 100 by default and at most 5000. Larger values are rejected. Default: `100`. Maximum: `5000`. |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 200 | `application/json` | PetList | The matching pets. |

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
