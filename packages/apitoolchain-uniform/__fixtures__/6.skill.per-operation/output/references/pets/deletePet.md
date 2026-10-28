<!-- Generated from the API reference. Do not edit by hand. -->

# `deletePet` — Delete pet

`DELETE /pets/{petId}`

Deletes a pet for good.

## Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `petId` | string | yes | The pet's ID. |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 204 | — | — | The pet was deleted. |
| 404 | `application/json` | Error | No pet has this ID. |

### 404 — `application/json`

| Name | Type | Description |
|---|---|---|
| `code` | string | A machine-readable error code. |
| `message` | string | A human-readable message. |

## Example request

```shell
curl --request DELETE \
     --url https://api.example.com/v1/pets/string \
     --header 'accept: application/json'
```
