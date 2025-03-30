from datetime import datetime
import time
import matplotlib.pyplot as plt
import requests
import sys
import matplotlib.dates as mdates
from io import BytesIO


def fetch_json(url):
    response = requests.get(url)
    return response.json()


def contest(CF_id):
    json = fetch_json("https://codeforces.com/api/user.rating?handle={}".format(CF_id))
    if json["status"] != "OK":
        return json["comment"]
    con = json["result"]
    if len(con) == 0:
        return "没有参赛记录"
    x = []
    y = []
    max_rating = 0
    for contest in con:
        y.append(contest["newRating"])
        max_rating = max(max_rating, contest["newRating"])
        s = time.strftime(
            "%y/%m/%d", time.gmtime(contest["ratingUpdateTimeSeconds"] + 3600 * 8)
        )
        x.append(s)

    plt.clf()
    plt.figure(dpi=300, figsize=(10, 5))

    plt.xlabel("Time")
    plt.ylabel("Rating")
    plt.title("{}'s Rating change".format(CF_id))

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
    plt.tick_params(axis="x", rotation=30)

    with BytesIO() as buffer:
        plt.savefig(buffer, format="png")
        buffer.seek(0)  # 在读取之前移动到缓冲区的开头
        sys.stdout.buffer.write(buffer.read())


if __name__ == "__main__":
    CF_id = sys.argv[1]
    contest(CF_id)
