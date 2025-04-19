import json
import os
import random
import sys
from tencentcloud.common import credential
from tencentcloud.common.profile.client_profile import ClientProfile
from tencentcloud.common.profile.http_profile import HttpProfile
from tencentcloud.common.exception.tencent_cloud_sdk_exception import (
    TencentCloudSDKException,
)
from tencentcloud.ocr.v20181119 import ocr_client, models


def main(url, secret_id, secret_key):
    try:
        # 实例化一个认证对象，入参需要传入腾讯云账户 SecretId 和 SecretKey，此处还需注意密钥对的保密
        # 代码泄露可能会导致 SecretId 和 SecretKey 泄露，并威胁账号下所有资源的安全性
        # 以下代码示例仅供参考，建议采用更安全的方式来使用密钥
        # 请参见：https://cloud.tencent.com/document/product/1278/85305
        # 密钥可前往官网控制台 https://console.cloud.tencent.com/cam/capi 进行获取
        cred = credential.Credential(secret_id, secret_key)
        # 实例化一个http选项，可选的，没有特殊需求可以跳过
        httpProfile = HttpProfile()
        httpProfile.endpoint = "ocr.tencentcloudapi.com"

        # 实例化一个client选项，可选的，没有特殊需求可以跳过
        clientProfile = ClientProfile()
        clientProfile.httpProfile = httpProfile
        # 实例化要请求产品的client对象,clientProfile是可选的
        client = ocr_client.OcrClient(cred, "", clientProfile)

        # 为了更好利用免费额度，随机选择一个接口
        flag = random.random() < 0.5

        # 实例化一个请求对象,每个接口都会对应一个request对象
        if flag:
            req = models.GeneralBasicOCRRequest()
        else:
            req = models.GeneralAccurateOCRRequest()

        params = {"ImageUrl": url}
        req.from_json_string(json.dumps(params))

        # 返回的resp是一个GeneralBasicOCRResponse的实例，与请求对象对应
        if flag:
            resp = client.GeneralBasicOCR(req)
        else:
            resp = client.GeneralAccurateOCR(req)
        # 输出json格式的字符串回包
        print(resp.to_json_string())

    except TencentCloudSDKException as err:
        print(err, file=sys.stderr)
        exit(-1)


if __name__ == "__main__":
    secret_id = os.getenv("SECRET_ID")
    secret_key = os.getenv("SECRET_KEY")
    url = os.getenv("IMAGE_URL")
    main(url, secret_id, secret_key)
