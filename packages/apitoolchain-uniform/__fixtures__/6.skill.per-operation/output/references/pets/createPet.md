<!-- Generated from the API reference. Do not edit by hand. -->

# `createPet` — Create pet

`POST /pets`

Creates a pet. Names need not be unique | pipes are kept.

## Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | The pet's name. Example: `Rex`. |
| `status` | string | no | One of: `available`, `pending`, `sold`. Default: `available`. |
| `tags` | array of string | no | Free-form labels. |
| `owner` | Owner | no | Who looks after the pet. |
| `owner.name` | string | yes | The owner's full name. |
| `owner.contact` | object | no | How to reach the owner. |
| `vaccinated` | boolean | no | Default: `false`. |
| `priority` | integer | no | Listing priority, 1 the highest. One of: `1`, `2`, `3`. |
| `size` | string or null | no | The pet's size; null when unknown. One of: `small`, `medium`, `large`, `null`. |
| `attributes` | object | no | Free-form attributes ==================== Stored as given, for example: ```json { "color": "brown", "legs": 4 } ``` Keys are case-sensitive. |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 201 | `application/json` | Pet | The pet that was created. |
| 422 | `application/json` | Error | The pet is invalid. |

### 201 — `application/json`

| Name | Type | Description |
|---|---|---|
| `id` | string | Unique identifier for the pet. |
| `name` | string | The pet's name. |
| `status` | string | Where the pet is in the sale. One of: `available`, `pending`, `sold`. |
| `tags` | array of string |  |
| `owner` | Owner | Who looks after the pet. |
| `owner.name` | string | The owner's full name. |
| `owner.contact` | object | How to reach the owner. |

## Example request

```shell
curl --request POST \
     --url https://api.example.com/v1/pets \
     --header 'accept: application/json' \
     --header 'content-type: application/json' \
     --data '
{
  "name": "Rex",
  "status": "available",
  "tags": [
    "string"
  ],
  "owner": {
    "name": "string",
    "contact": {
      "email": "string",
      "phone": "string"
    }
  },
  "vaccinated": false,
  "priority": 1,
  "size": "small"
}
'
```
