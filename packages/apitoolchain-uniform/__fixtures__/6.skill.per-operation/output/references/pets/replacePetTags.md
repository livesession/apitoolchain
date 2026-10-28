<!-- Generated from the API reference. Do not edit by hand. -->

# `replacePetTags` — Replace pet tags

`PUT /pets/{petId}/tags`

Replaces every tag of a pet with the list given.

## Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `petId` | string | yes | The pet's ID. |

## Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| (body) | array | yes |  |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 200 | `application/json` | Pet | The pet with its new tags. |

### 200 — `application/json`

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
curl --request PUT \
     --url https://api.example.com/v1/pets/string/tags \
     --header 'accept: application/json' \
     --header 'content-type: application/json' \
     --data '["string"]'
```
