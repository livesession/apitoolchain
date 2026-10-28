<!-- Generated from the API reference. Do not edit by hand. -->

# `updatePet` — Update pet

`PATCH /pets/{petId}`

Updates the fields given; the others keep their values.

## Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `petId` | string | yes | The pet's ID. |

## Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `name` | string | no | The new name. |
| `status` | string | no | One of: `available`, `pending`, `sold`. |
| `weight` | number or null | no | Weight in kilograms; null clears it. |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 200 | `application/json` | Pet | The updated pet. |

## Example request

```shell
curl --request PATCH \
     --url https://api.example.com/v1/pets/string \
     --header 'accept: application/json' \
     --header 'content-type: application/json' \
     --data '
{
  "name": "string",
  "status": "available",
  "weight": 0
}
'
```
