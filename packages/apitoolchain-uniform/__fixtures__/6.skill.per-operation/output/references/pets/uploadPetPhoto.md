<!-- Generated from the API reference. Do not edit by hand. -->

# `uploadPetPhoto` — Upload pet photo

`POST /pets/{petId}/photo`

Uploads a photo of the pet.

## Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `petId` | string | yes | The pet's ID. |

## Request body

Content type: `multipart/form-data`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `file` | string | yes | The image. |
| `caption` | string | no |  |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 204 | — | — | The photo was stored. |

## Example request

```shell
curl --request POST \
     --url https://api.example.com/v1/pets/string/photo \
     --header 'content-type: multipart/form-data' \
     --form file=string \
     --form caption=string
```
