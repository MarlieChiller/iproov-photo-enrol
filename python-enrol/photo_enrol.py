import argparse
import sys

import requests as r
import requests.auth
from pydantic_settings import BaseSettings
from pydantic import Field
import friendlywords as fw

import logging
from rich.logging import RichHandler


class Settings(BaseSettings):
    LOG_LEVEL: str
    region: str = Field(alias="REGION")
    img_src: str = Field(alias="IMAGE_SOURCE")
    img_path: str = Field(alias="IMAGE_PATH")
    sp_key: str = Field(alias="SP_KEY")
    sp_secret: str = Field(alias="SP_SECRET")
    oa_username: str = Field(alias="OAUTH_USERNAME")
    oa_pw: str = Field(alias="OAUTH_PW")


settings = Settings(_env_file="../.env", _env_file_encoding="utf-8")
FORMAT = "%(message)s"
logging.basicConfig(
    level=settings.LOG_LEVEL.upper(),
    format=FORMAT,
    datefmt="[%X]",
    handlers=[RichHandler()],
)

log = logging.getLogger("rich")


def request_log(req, msg):
    if req.status_code == 200:
        log.info(f"{msg} succeeded")
    else:
        log.error(f"{msg} failed. {req.status_code} {req.text}")
        sys.exit()


def create_token(config: Settings, username: str) -> str:
    enrol_token_url = (
        f"https://{config.region}.secure.iproov.me/api/v2/claim/enrol/token"
    )
    enrol_token_body = {
        "resource": "photo_enrol_test",
        "api_key": config.sp_key,
        "secret": config.sp_secret,
        "user_id": username,
    }
    log.debug(f"getting enrol token, url={enrol_token_url}, body={enrol_token_body}")
    get_token = r.post(url=enrol_token_url, json=enrol_token_body)
    request_log(get_token, "create token")
    return get_token.json()["token"]


def send_photo(config: Settings, token: str):
    with open(config.img_path, "rb") as f:
        image = f.read()

    enrol_image_url = (
        f"https://{config.region}.secure.iproov.me/api/v2/claim/enrol/image"
    )
    enrol_image_body = {
        "api_key": (None, config.sp_key),
        "secret": (None, config.sp_secret),
        "rotation": (None, "0"),
        "image": ("image", image),
        "token": (None, token),
        "source": (None, config.img_src),
    }

    log.debug(
        f"sending image for enrolment, url={enrol_image_url}, body={enrol_image_body.keys()}"
    )
    enrol_image = r.post(url=enrol_image_url, files=enrol_image_body)
    request_log(enrol_image, "enrol image")


def create_access_token(config: Settings) -> str:
    url = (
        f"https://{config.region}.secure.iproov.me/api/v2/{config.sp_key}/access_token"
    )
    enrol_image_body = {"grant_type": "client_credentials"}
    client_auth = r.auth.HTTPBasicAuth(config.oa_username, config.oa_pw)
    log.debug("getting oauth access token")
    acc_token_response = r.post(url=url, data=enrol_image_body, auth=client_auth)
    request_log(acc_token_response, "generate access token")
    return acc_token_response.json()["access_token"]


def delete_user(config: Settings, access_token: str, username: str):
    url4 = f"https://{config.region}.secure.iproov.me/api/v2/users/{username}"
    header = {"Authorization": f"Bearer {access_token}"}
    log.debug("deleting user")
    user_deleted = r.delete(url=url4, headers=header)
    request_log(user_deleted, "delete user")


def photo_enrol(args, config: Settings):
    username = fw.generate(3, separator="_")
    token = create_token(config, username)
    send_photo(config, token)
    if args.delete_user:
        access_token = create_access_token(config)
        delete_user(config, access_token, username)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        prog="Photo Enrol", description="Python script for photo enrol"
    )
    parser.add_argument(
        "-d",
        "--delete_user",
        help="deletes the user after enrolment",
        action="store_true",
    )
    args = parser.parse_args()

    photo_enrol(args, settings)
