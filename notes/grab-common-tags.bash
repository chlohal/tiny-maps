for i in {1..20} 
do
    curl "https://taginfo.openstreetmap.org/api/4/keys/all?include=none&sortname=count_nodes&sortorder=desc&rp=27&page=$i" | jq -r '.data[] | .key'
done > osm_common_tags