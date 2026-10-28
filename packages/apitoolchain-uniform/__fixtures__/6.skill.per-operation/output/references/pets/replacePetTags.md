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

## Example request

```shell
curl --request PUT \
     --url https://api.example.com/v1/pets/string/tags \
     --header 'accept: application/json' \
     --header 'content-type: application/json' \
     --data '["string"]'
```
