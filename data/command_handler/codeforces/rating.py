from datetime import datetime
import time
import matplotlib.pyplot as plt
import requests
import sys
import matplotlib.dates as mdates
from io import BytesIO


def fetch_json(url):
    try:
        response = requests.get(url)
    except:
        print("无法连接到 Codeforces 服务器", file=sys.stderr)
        exit(-1)
    return response.json()


def contest(CF_id):
    json = fetch_json("https://codeforces.com/api/user.rating?handle={}".format(CF_id))
    if json["status"] != "OK":
        print(json["comment"], file=sys.stderr)
        exit(-1)
    con = json["result"]
    if len(con) == 0:
        print("没有参赛记录", file=sys.stderr)
        exit(-1)
    x = []
    y = []
    max_rating = float("-inf")
    min_rating = float("inf")
    for contest in con:
        rating = contest["newRating"]
        y.append(rating)
        max_rating = max(max_rating, rating)
        min_rating = min(min_rating, rating)
        s = time.strftime(
            "%y/%m/%d", time.gmtime(contest["ratingUpdateTimeSeconds"] + 3600 * 8)
        )
        x.append(s)

    plt.clf()
    plt.xlabel("Time")
    plt.ylabel("Rating")

    _, ax = plt.subplots(dpi=300, figsize=(10, 5))

    # 计算纵坐标显示范围
    gap = (max_rating - min_rating) * 0.1

    y_min = min_rating - gap
    y_max = max_rating + gap
    ax.set_ylim(y_min, y_max)

    ax.axhspan(y_min, 1200, facecolor="#808080", alpha=0.5)  # 灰色
    ax.axhspan(1200, 1400, facecolor="#008000", alpha=0.5)  # 绿色
    ax.axhspan(1400, 1600, facecolor="#00C0C0", alpha=0.5)  # 青色
    ax.axhspan(1600, 1900, facecolor="#0000FF", alpha=0.5)  # 蓝色
    ax.axhspan(1900, 2100, facecolor="#800080", alpha=0.5)  # 紫色
    ax.axhspan(2100, 2400, facecolor="#FFA500", alpha=0.5)  # 橙色
    ax.axhspan(2400, y_max, facecolor="#FF0000", alpha=0.5)  # 红色

    plt.title("{}'s rating change".format(CF_id))

    date = [datetime.strptime(s, "%y/%m/%d") for s in x]
    plt.gca().xaxis.set_major_formatter(mdates.DateFormatter("%y-%m-%d"))
    plt.plot(
        date,
        y,
        "o-",
        color="#4169E1",
        alpha=0.8,
        linewidth=1,
        label="rating",
        markersize=2,
    )
    plt.legend()
    plt.tick_params(axis="x", rotation=20)

    with BytesIO() as buffer:
        plt.savefig(buffer, format="png")
        buffer.seek(0)  # 在读取之前移动到缓冲区的开头
        sys.stdout.buffer.write(buffer.read())


if __name__ == "__main__":
    CF_id = sys.argv[1]
    contest(CF_id)
