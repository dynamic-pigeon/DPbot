import matplotlib.pyplot as plt
import requests
from io import BytesIO
import sys


def fetch_json(url):
    try:
        response = requests.get(url)
    except:
        print("无法连接到 Codeforces 服务器", file=sys.stderr)
        exit(-1)
    return response.json()


def analyze(CF_id):
    json = fetch_json("https://codeforces.com/api/user.status?handle={}".format(CF_id))
    if json["status"] != "OK":
        print(json["comment"], file=sys.stderr)
        exit(-1)
    status = json["result"]
    if len(status) == 0:
        print("没有提交记录", file=sys.stderr)
        exit(-1)
    AC_status = []
    vis = set()
    for x in status:
        if (
            "problem" not in x.keys()
            or "problemsetName" in x["problem"]
            or "verdict" not in x.keys()
        ):
            continue
        if (
            x["verdict"] == "OK"
            and (str(x["problem"]["contestId"]) + x["problem"]["index"]) not in vis
        ):
            AC_status.append(x["problem"])
            vis.add(str(x["problem"]["contestId"]) + x["problem"]["index"])
    plt.clf()
    color = (
        ["gray"] * 4
        + ["g"] * 2
        + ["c"] * 2
        + ["b"] * 3
        + ["purple"] * 2
        + ["orange"] * 3
        + ["red"] * 12
    )
    y = [0] * 28
    for t in AC_status:
        if "rating" in t:
            y[t["rating"] // 100 - 8] += 1
    x = [i for i in range(800, 3600, 100)]
    bar_width = 96
    plt.figure(dpi=300, figsize=(10, 5))
    plt.xlim(750, 3550)
    for i in range(len(y)):
        plt.bar(
            x[i], y[i], bar_width, color=color[i], edgecolor=color[i], antialiased=True
        )
    # plt.show()
    plt.title("{} solved {} problems in total".format(CF_id, len(AC_status)))
    plt.xlabel("Rating")
    plt.ylabel("Frequency")

    with BytesIO() as buffer:
        plt.savefig(buffer, format="png")
        buffer.seek(0)  # 在读取之前移动到缓冲区的开头
        sys.stdout.buffer.write(buffer.read())


if __name__ == "__main__":
    CF_id = sys.argv[1]
    analyze(CF_id)
