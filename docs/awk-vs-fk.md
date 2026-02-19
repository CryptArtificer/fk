# awk vs fk — 100 array programs compared

Side-by-side: clunky awk on the left, clean fk on the right.

---

## 1. Unique values

```sh
# 1. Print unique lines (preserve order)
awk '!seen[$0]++' file
fk  '!seen[$0]++' file                          # same

# 2. Print sorted unique values of column 1
awk '{ a[$1] } END { n=asorti(a,b); for(i=1;i<=n;i++) print b[i] }' file
fk  '{ a[$1]++ } END { print a }' file

# 3. Sorted unique with count
awk '{ a[$1]++ } END { for(k in a) print a[k], k | "sort -rn" }' file
fk  '{ a[$1]++ } END { for(k in a) print a[k], k }' file   # same (pipe to sort -rn)

# 4. Unique values from column 3, comma-separated
awk '{ a[$3] } END { s=""; for(k in a) { if(s!="") s=s","; s=s k } print s }' file
fk  '{ a[$3]++ } END { print join(a) }' file

# 5. Count distinct values
awk '{ a[$1] } END { n=0; for(k in a) n++; print n }' file
fk  '{ a[$1]++ } END { print length(a) }' file  # same as awk, but also:
fk  '{ a[$1]++ } END { print keys(a) }' file    # see them too

# 6. Unique lines from multiple columns
awk '{ k=$1 OFS $2; if(!(k in seen)) { seen[k]; print } }' file
fk  '!seen[$1,$2]++' file                        # same

# 7. Top N unique values by frequency
awk '{ a[$1]++ } END { for(k in a) print a[k],k }' file | sort -rn | head -5
fk  '{ a[$1]++ } END { for(k in a) print a[k],k }' file | sort -rn | head -5  # same
```

## 2. Sorting

```sh
# 8. Sort array values alphabetically, print
awk '{ a[NR]=$0 } END { n=asort(a); for(i=1;i<=n;i++) print a[i] }' file
fk  '{ a[NR]=$0 } END { asort(a); print a }' file

# 9. Sort by keys, print keys
awk '{ a[$1]=$0 } END { n=asorti(a,k); for(i=1;i<=n;i++) print k[i] }' file
fk  '{ a[$1]=$0 } END { asorti(a); print a }' file

# 10. Sort CSV column and output
awk -F, '{ a[NR]=$3 } END { n=asort(a); for(i=1;i<=n;i++) print a[i] }' file
fk  -i csv '{ a[NR]=$3 } END { asort(a); print a }' file

# 11. Reverse sort (descending)
awk '{ a[NR]=$1 } END { n=asort(a); for(i=n;i>=1;i--) print a[i] }' file
fk  '{ a[NR]=$1 } END { asort(a); for(i=length(a);i>=1;i--) print a[i] }' file

# 12. Sort and deduplicate
awk '{ a[$1] } END { n=asorti(a,k); for(i=1;i<=n;i++) print k[i] }' file
fk  '{ a[$1]++ } END { print a }' file

# 13. Sort array, join with comma
awk '{ a[NR]=$1 } END { n=asort(a); s=""; for(i=1;i<=n;i++){if(s!="")s=s",";s=s a[i]} print s }' file
fk  '{ a[NR]=$1 } END { asort(a); print join(a, ",") }' file
```

## 3. Frequency counting & display

```sh
# 14. Basic frequency count
awk '{ a[$1]++ } END { for(k in a) print a[k], k }' file
fk  '{ a[$1]++ } END { for(k in a) print a[k], k }' file   # same

# 15. Sorted frequency table
awk '{ a[$1]++ } END { for(k in a) print a[k], k }' file | sort -rn
fk  '{ a[$1]++ } END { inv(a); asort(a); print a }' file    # sorted by count... or pipe

# 16. Histogram with bars
awk '{ a[$1]++ } END { for(k in a) { printf "%-15s ", k; for(i=0;i<a[k];i++) printf "#"; print "" } }' file
fk  '{ a[$1]++ } END { for(k in a) printf "%-15s %s\n", k, repeat("#",a[k]) }' file

# 17. Percentage breakdown
awk '{ a[$1]++; t++ } END { for(k in a) printf "%s: %.1f%%\n", k, a[k]/t*100 }' file
fk  '{ a[$1]++; t++ } END { for(k in a) printf "%s: %.1f%%\n", k, a[k]/t*100 }' file

# 18. Top 3 most frequent
awk '{ a[$1]++ } END { for(k in a) print a[k],k }' file | sort -rn | head -3
fk  '{ a[$1]++ } END { for(k in a) print a[k],k }' file | sort -rn | head -3

# 19. Word frequency from all fields
awk '{ for(i=1;i<=NF;i++) a[$i]++ } END { for(k in a) print a[k], k }' file
fk  '{ for(i=1;i<=NF;i++) a[$i]++ } END { for(k in a) print a[k], k }' file
```

## 4. Set operations

```sh
# 20. Lines in file1 but not file2
awk 'NR==FNR{a[$0];next} !($0 in a)' file2 file1
fk  'NR==FNR{a[$0];next} !($0 in a)' file2 file1  # same, or:
fk  'NR==FNR{a[$0]++;next}{b[$0]++} END{diff(b,a);print b}' file1 file2

# 21. Lines common to both files
awk 'NR==FNR{a[$0];next} $0 in a' file2 file1
fk  'NR==FNR{a[$0]++;next}{b[$0]++} END{inter(b,a);print b}' file1 file2

# 22. Union of two files (all unique lines)
awk '!seen[$0]++' file1 file2
fk  '!seen[$0]++' file1 file2                   # same, or:
fk  '{a[$0]++}' file1 file2 | fk 'END{print a}' # with print arr

# 23. Symmetric difference (in one but not both)
awk 'NR==FNR{a[$0]++;next} {if($0 in a) delete a[$0]; else b[$0]} END{for(k in a)print k;for(k in b)print k}' f1 f2
fk  'NR==FNR{a[$0]++;next}{b[$0]++} END{x=a;y=b;diff(a,y);diff(b,x);union(a,b);print a}' f1 f2

# 24. Column 1 values in file1 not in file2
awk -F, 'NR==FNR{a[$1];next} !($1 in a)' file2.csv file1.csv
fk  -i csv 'NR==FNR{a[$1]++;next} !($1 in a)' file2.csv file1.csv

# 25. Intersection count
awk 'NR==FNR{a[$1];next} $1 in a{c++} END{print c+0}' f2 f1
fk  'NR==FNR{a[$1]++;next}{b[$1]++} END{inter(a,b);print length(a)}' f1 f2

# 26. Three-way intersection
awk 'FILENAME==ARGV[1]{a[$0];next} FILENAME==ARGV[2]{if($0 in a)b[$0];next} $0 in b' f1 f2 f3
fk  'FNR==1&&NR>FNR{fc++} fc==0{a[$0]++;next} fc==1{b[$0]++;next} {c[$0]++} END{inter(a,b);inter(a,c);print a}' f1 f2 f3

# 27. Users in /etc/passwd but not in active_users.txt
awk -F: 'NR==FNR{a[$1];next} !($1 in a){print $1}' active.txt /etc/passwd
fk  -F: 'NR==FNR{a[$1]++;next}{b[$1]++} END{diff(b,a);print b}' active.txt /etc/passwd
```

## 5. Array manipulation

```sh
# 28. Reverse an array
awk '{ a[NR]=$0 } END { for(i=NR;i>=1;i--) print a[i] }' file
fk  '{ a[NR]=$0 } END { for(i=NR;i>=1;i--) print a[i] }' file  # same

# 29. Remove duplicates from collected values
awk '{ a[NR]=$1 } END { delete seen; n=0; for(i=1;i<=NR;i++) if(!(a[i] in seen)){seen[a[i]];b[++n]=a[i]} for(i=1;i<=n;i++)print b[i] }' f
fk  '{ a[NR]=$1 } END { uniq(a); print a }' file

# 30. Swap key-value in associative array
awk '{ a[$1]=$2 } END { for(k in a) b[a[k]]=k; for(k in b) print k, b[k] }' file
fk  '{ a[$1]=$2 } END { inv(a); for(k in a) print k, a[k] }' file

# 31. Remove empty entries
awk '{ a[NR]=$1 } END { n=0; for(i=1;i<=NR;i++) if(a[i]!="") b[++n]=a[i]; for(i=1;i<=n;i++) print b[i] }' file
fk  '{ a[NR]=$1 } END { tidy(a); print a }' file

# 32. Collect then sort values from specific field
awk '$2=="error" { a[NR]=$3 } END { n=asort(a); for(i=1;i<=n;i++) print a[i] }' log
fk  '$2=="error" { a[NR]=$3 } END { asort(a); print a }' log

# 33. Flatten multi-value field into unique set
awk -F, '{ split($3,tmp,";"); for(i in tmp) a[tmp[i]] } END { for(k in a) print k }' file
fk  -F, '{ split($3,tmp,";"); for(i in tmp) a[tmp[i]]++ } END { print a }' file

# 34. Build lookup table, then print all keys
awk '{ map[$1]=$2 } END { for(k in map) print k }' lookup.txt
fk  '{ map[$1]=$2 } END { print keys(map) }' lookup.txt

# 35. Build lookup, print all values
awk '{ map[$1]=$2 } END { for(k in map) print map[k] }' lookup.txt
fk  '{ map[$1]=$2 } END { print vals(map) }' lookup.txt

# 36. Merge two arrays
awk 'NR==FNR{a[$1]=$2;next}{b[$1]=$2} END{for(k in b)a[k]=b[k];for(k in a)print k,a[k]}' f1 f2
fk  'NR==FNR{a[$1]=$2;next}{b[$1]=$2} END{union(a,b);for(k in a) print k,a[k]}' f1 f2
```

## 6. Statistical analysis

```sh
# 37. Mean of column 1
awk '{ s+=$1 } END { print s/NR }' file
fk  '{ s+=$1 } END { print s/NR }' file         # same, or:
fk  '{ a[NR]=$1 } END { print mean(a) }' file

# 38. Median
awk '{ a[NR]=$1 } END { n=asort(a); if(n%2) print a[int(n/2)+1]; else print (a[n/2]+a[n/2+1])/2 }' f
fk  '{ a[NR]=$1 } END { print median(a) }' file

# 39. Standard deviation
awk '{ a[NR]=$1;s+=$1 } END { m=s/NR;ss=0;for(i=1;i<=NR;i++)ss+=(a[i]-m)^2;print sqrt(ss/NR) }' f
fk  '{ a[NR]=$1 } END { print stddev(a) }' file

# 40. Full summary stats
awk '{ a[NR]=$1;s+=$1 } END { m=s/NR;for(i=1;i<=NR;i++)v+=(a[i]-m)^2;n=asort(a);print "n="n,"mean="m,"sd="sqrt(v/n),"min="a[1],"max="a[n] }' f
fk  '{ a[NR]=$1 } END { printf "n=%d mean=%.2f sd=%.2f min=%s max=%s\n", length(a), mean(a), stddev(a), min(a), max(a) }' f

# 41. Percentile (p95)
awk '{ a[NR]=$1 } END { n=asort(a); i=int(n*0.95); if(i<1)i=1; print a[i] }' file
fk  '{ a[NR]=$1 } END { print p(a, 95) }' file

# 42. Group-by mean
awk '{ s[$1]+=$2; c[$1]++ } END { for(k in s) print k, s[k]/c[k] }' file
fk  '{ s[$1]+=$2; c[$1]++ } END { for(k in s) print k, s[k]/c[k] }' file  # same

# 43. Interquartile mean
awk '{ a[NR]=$1 } END { n=asort(a); q1=int(n*0.25)+1; q3=int(n*0.75); s=0;c=0; for(i=q1;i<=q3;i++){s+=a[i];c++} print s/c }' f
fk  '{ a[NR]=$1 } END { print iqm(a) }' file

# 44. Variance
awk '{ a[NR]=$1;s+=$1 } END { m=s/NR;v=0;for(i=1;i<=NR;i++)v+=(a[i]-m)^2;print v/NR }' file
fk  '{ a[NR]=$1 } END { print variance(a) }' file
```

## 7. File and I/O operations

```sh
# 45. Read lookup file into array
awk 'NR==FNR{a[$1]=$2;next} ($1 in a){print $0, a[$1]}' lookup.txt data.txt
fk  'NR==FNR{a[$1]=$2;next} ($1 in a){print $0, a[$1]}' lookup.txt data.txt  # same

# 46. Slurp a config file into memory
awk 'BEGIN{while((getline line < "config.txt")>0){split(line,p,"=");cfg[p[1]]=p[2]}} {print cfg[$1]}' data
fk  'BEGIN{slurp("config.txt",L);for(i=1;i<=length(L);i++){split(L[i],p,"=");cfg[p[1]]=p[2]}} {print cfg[$1]}' data

# 47. Read entire file as string
awk 'BEGIN{RS="";getline content < "template.txt"} {gsub("NAME",$1,content);print content}' data
fk  'BEGIN{tmpl=slurp("template.txt")} {print gensub("NAME",$1,"g",tmpl)}' data

# 48. Count lines in external file
awk 'BEGIN{n=0;while((getline<"other.txt")>0)n++;print n}'
fk  'BEGIN{print slurp("other.txt",a)}' 

# 49. Load blocklist, filter input
awk 'BEGIN{while((getline<"block.txt")>0)bl[$0]} !($1 in bl)' data.txt
fk  'BEGIN{slurp("block.txt",L);for(i in L)bl[L[i]]++} !($1 in bl)' data.txt

# 50. Read two files, cross-reference
awk 'BEGIN{while((getline<"ids.txt")>0)ids[$1]} NR==FNR{next} $2 in ids' skip.txt data.txt
fk  'BEGIN{slurp("ids.txt",L);for(i in L)ids[L[i]]++} $2 in ids' data.txt
```

## 8. String formatting

```sh
# 51. Right-align numbers in column
awk '{ printf "%10s %s\n", $1, $2 }' file
fk  '{ print lpad($1, 10), $2 }' file

# 52. Left-align names in fixed width
awk '{ printf "%-20s %s\n", $1, $2 }' file
fk  '{ print rpad($1, 20), $2 }' file

# 53. Zero-pad IDs
awk '{ printf "%06d %s\n", $1, $2 }' file
fk  '{ print lpad($1, 6, "0"), $2 }' file

# 54. Create a simple table
awk 'BEGIN{printf "%-15s %10s %10s\n","Name","Score","Grade"} {printf "%-15s %10d %10s\n",$1,$2,$3}' f
fk  'BEGIN{print rpad("Name",15), lpad("Score",10), lpad("Grade",10)} {print rpad($1,15), lpad($2,10), lpad($3,10)}' f

# 55. Dot-leader formatting
awk '{ n=40-length($1)-length($2); s=""; for(i=0;i<n;i++)s=s"."; print $1 s $2 }' file
fk  '{ print rpad($1, 40-length($2), ".") $2 }' file

# 56. Box drawing
awk 'BEGIN{s="";for(i=1;i<=40;i++)s=s"-";print "+"s"+"} {printf "| %-38s |\n",$0} END{print "+"s"+"}' f
fk  'BEGIN{print "+" repeat("-",40) "+"} {print "| " rpad($0,38) " |"} END{print "+" repeat("-",40) "+"}' f

# 57. Indent with custom prefix
awk '{ printf "    → %s\n", $0 }' file
fk  '{ print lpad("", 4) "→ " $0 }' file

# 58. Truncate long strings with ellipsis
awk '{ s=$0; if(length(s)>30) s=substr(s,1,27)"..."; print s }' file
fk  '{ s=$0; if(length(s)>30) s=substr(s,1,27)"..."; print s }' file  # same
```

## 9. Data transformation

```sh
# 59. Transpose rows to columns (simple)
awk '{ for(i=1;i<=NF;i++) a[i][NR]=$i } END { for(i=1;i<=length(a);i++){s="";for(j=1;j<=NR;j++){if(s!="")s=s OFS;s=s a[i][j]}print s} }' f
fk  '{ for(i=1;i<=NF;i++) a[i,NR]=$i } END { for(i=1;i<=NF;i++){for(j=1;j<=NR;j++){if(j>1)printf OFS;printf "%s",a[i,j]}print""} }' f

# 60. Pivot: group by col1, collect col2 values
awk '{ a[$1]=a[$1] (a[$1]?",":"") $2 } END { for(k in a) print k, a[k] }' file
fk  '{ a[$1]=a[$1] (a[$1]?",":"") $2 } END { for(k in a) print k, a[k] }' file  # same

# 61. Deduplicate and sort a comma-separated field
awk '{ split($1,arr,","); delete seen; s=""; n=asort(arr); for(i=1;i<=n;i++) if(!(arr[i] in seen)){seen[arr[i]];if(s!="")s=s",";s=s arr[i]} print s }' f
fk  '{ split($1,arr,","); uniq(arr); asort(arr); print join(arr,",") }' file

# 62. Normalize whitespace-separated tags
awk '{ delete a; for(i=1;i<=NF;i++) a[$i]; s=""; n=asorti(a,b); for(i=1;i<=n;i++){if(s!="")s=s" ";s=s b[i]} print s }' file
fk  '{ for(i=1;i<=NF;i++) a[$i]++; print keys(a); delete a }' file

# 63. Group-by with sorted keys output
awk '{ a[$1]+=$2 } END { n=asorti(a,k); for(i=1;i<=n;i++) print k[i], a[k[i]] }' file
fk  '{ a[$1]+=$2 } END { asorti(a); for(i=1;i<=length(a);i++) print a[i] }' file  # loses values
fk  '{ s[$1]+=$2 } END { for(k in s) print k, s[k] }' file   # keys auto-sorted in print

# 64. Collect fields into array, join with pipe
awk '{ a[NR]=$3 } END { s=""; for(i=1;i<=NR;i++){if(s!="")s=s"|";s=s a[i]} print s }' file
fk  '{ a[NR]=$3 } END { print join(a, "|") }' file

# 65. Replace values using lookup map
awk 'NR==FNR{m[$1]=$2;next} {for(i=1;i<=NF;i++) if($i in m) $i=m[$i]; print}' map.txt data.txt
fk  'NR==FNR{m[$1]=$2;next} {for(i=1;i<=NF;i++) if($i in m) $i=m[$i]; print}' map.txt data.txt  # same

# 66. Explode array back to lines
awk '{ split($0,a,","); for(i=1;i<=length(a);i++) print a[i] }' file
fk  '{ split($0,a,","); print vals(a) }' file

# 67. Build CSV from arrays
awk '{ a[NR]=$1; b[NR]=$2 } END { for(i=1;i<=NR;i++) print a[i]","b[i] }' file
fk  '{ a[NR]=$1; b[NR]=$2 } END { for(i=1;i<=NR;i++) print a[i]","b[i] }' file  # same
```

## 10. Multi-file processing

```sh
# 68. Enrich from lookup file
awk 'NR==FNR { lu[$1]=$2; next } { print $0, ($1 in lu ? lu[$1] : "N/A") }' ref.txt data.txt
fk  'NR==FNR { lu[$1]=$2; next } { print $0, ($1 in lu ? lu[$1] : "N/A") }' ref.txt data.txt

# 69. Anti-join: print lines from data not in blocklist
awk 'NR==FNR{a[$0];next} !($0 in a)' block.txt data.txt
fk  'NR==FNR{a[$0]++;next} !($0 in a)' block.txt data.txt

# 70. Semi-join: filter data by keys in keyfile
awk 'NR==FNR{a[$1];next} $1 in a' keys.txt data.txt
fk  'NR==FNR{a[$1]++;next} $1 in a' keys.txt data.txt

# 71. Full outer join (all keys from both)
awk 'NR==FNR{a[$1]=$2;next} {b[$1]=$2} END{for(k in a)print k,a[k],(k in b?b[k]:"");for(k in b)if(!(k in a))print k,"",b[k]}' f1 f2
fk  'NR==FNR{a[$1]=$2;next}{b[$1]=$2} END{for(k in a)print k,a[k],(k in b?b[k]:"");for(k in b)if(!(k in a))print k,"",b[k]}' f1 f2

# 72. Compare two configs, show differences
awk 'NR==FNR{a[$1]=$2;next} $1 in a && a[$1]!=$2{print $1,"old="a[$1],"new="$2}' old.cfg new.cfg
fk  'NR==FNR{a[$1]=$2;next} $1 in a && a[$1]!=$2{print $1,"old="a[$1],"new="$2}' old.cfg new.cfg

# 73. Count per-file unique values
awk '{ a[FILENAME,$1]++ } END { for(k in a) { split(k,p,SUBSEP); files[p[1]]++ } for(f in files) print f, files[f] }' f1 f2
fk  '{ a[FILENAME,$1]++ } END { for(k in a) { split(k,p,SUBSEP); files[p[1]]++ } for(f in files) print f, files[f] }' f1 f2
```

## 11. Random and sampling

```sh
# 74. Reservoir sampling — 10 random lines
awk 'BEGIN{srand()} {a[NR]=$0} NR<=10{next} {i=int(rand()*NR)+1; if(i<=10)a[i]=$0} END{for(i=1;i<=10;i++)print a[i]}' file
fk  '{ a[NR]=$0 } END { samp(a, 10); print a }' file

# 75. Shuffle all lines
awk 'BEGIN{srand()} {a[NR]=$0} END{for(i=NR;i>1;i--){j=int(rand()*i)+1;t=a[i];a[i]=a[j];a[j]=t}for(i=1;i<=NR;i++)print a[i]}' file
fk  '{ a[NR]=$0 } END { shuf(a); print a }' file

# 76. Random 20% sample
awk 'BEGIN{srand()} rand()<0.2' file
fk  'rand()<0.2' file                            # same, or exact count:
fk  '{ a[NR]=$0 } END { samp(a, int(length(a)*0.2)); print a }' file

# 77. Deal 5 random cards from 52
awk 'BEGIN{srand();for(i=1;i<=52;i++)d[i]=i;for(i=52;i>47;i--){j=int(rand()*i)+1;t=d[i];d[i]=d[j];d[j]=t;print d[i]}}'
fk  'BEGIN{srand();seq(d,1,52);samp(d,5);print d}'

# 78. Bootstrap sample (sample with replacement — approximate)
awk 'BEGIN{srand()} {a[NR]=$0} END{for(i=1;i<=NR;i++){j=int(rand()*NR)+1;print a[j]}}' file
fk  'BEGIN{srand()} {a[NR]=$0} END{for(i=1;i<=NR;i++){j=int(rand()*NR)+1;print a[j]}}' file  # same

# 79. Random permutation of column values
awk 'BEGIN{srand()} {a[NR]=$1;rest[NR]=$0} END{for(i=NR;i>1;i--){j=int(rand()*i)+1;t=a[i];a[i]=a[j];a[j]=t}for(i=1;i<=NR;i++)print a[i]}' f
fk  '{ a[NR]=$1 } END { shuf(a); print a }' file
```

## 12. Complex real-world patterns

```sh
# 80. CSV: unique emails, sorted
awk -F, 'NR>1{a[$3]} END{n=asorti(a,b);for(i=1;i<=n;i++)print b[i]}' users.csv
fk  -i csv -H '{ a[$email]++ } END { print a }' users.csv

# 81. Log analysis: unique IPs sorted
awk '{a[$1]} END{n=asorti(a,b);for(i=1;i<=n;i++)print b[i]}' access.log
fk  '{a[$1]++} END{print a}' access.log

# 82. Top error messages with count, formatted
awk '/ERROR/{a[$NF]++} END{for(k in a) printf "%6d %s\n",a[k],k}' log | sort -rn | head
fk  '/ERROR/{a[$NF]++} END{for(k in a) printf "%6d %s\n", a[k], k}' log | sort -rn | head

# 83. Session duration: first/last timestamp per user
awk '{if(!($1 in first))first[$1]=$2;last[$1]=$2} END{for(u in first)print u,first[u],last[u]}' log
fk  '{if(!($1 in first))first[$1]=$2;last[$1]=$2} END{for(u in first)print u,first[u],last[u]}' log

# 84. Deduplicate JSON lines by ID field
awk -F'"' '{ for(i=1;i<=NF;i++) if($i=="id") { id=$(i+2); if(!(id in seen)){seen[id]; print} break } }' data.jsonl
fk  -i json '!seen[$1]++' data.jsonl

# 85. Cross-tabulation: count of (col1, col2) pairs
awk '{ a[$1 FS $2]++ } END { for(k in a) print k, a[k] }' file
fk  '{ a[$1,$2]++ } END { for(k in a) print k, a[k] }' file

# 86. Running distinct count
awk '{ seen[$1]; n=0; for(k in seen) n++; print NR, n }' file
fk  '{ seen[$1]++; print NR, length(seen) }' file

# 87. Collect, deduplicate, sort tags per group
awk '{ split($2,t,","); for(i in t) a[$1][t[i]]; } END { for(g in a){s="";n=asorti(a[g],k);for(i=1;i<=n;i++){if(s!="")s=s",";s=s k[i]}print g,s} }' f
fk  '{ split($2,t,","); for(i in t) tags[$1,t[i]]++ } END { for(k in tags) { split(k,p,SUBSEP); grp[p[1]]=grp[p[1]] (grp[p[1]]?",":"") p[2] } for(g in grp) print g, grp[g] }' f

# 88. Sliding window average (window=3)
awk '{ a[NR]=$1 } NR>=3 { print (a[NR]+a[NR-1]+a[NR-2])/3 }' file
fk  '{ a[NR]=$1 } NR>=3 { print (a[NR]+a[NR-1]+a[NR-2])/3 }' file  # same

# 89. Rank values (dense ranking)
awk '{ a[NR]=$1 } END { n=asort(a,s); r=0;prev=""; for(i=1;i<=n;i++){if(s[i]!=prev)r++;rank[s[i]]=r;prev=s[i]} }' file
fk  '{ a[NR]=$1 } END { asort(a); r=0;prev=""; for(i=1;i<=length(a);i++){if(a[i]!=prev)r++;print a[i],r;prev=a[i]} }' file

# 90. Mode (most frequent value)
awk '{ a[$1]++ } END { max=0; for(k in a) if(a[k]>max){max=a[k];mode=k} print mode }' file
fk  '{ a[$1]++ } END { max=0; for(k in a) if(a[k]>max){max=a[k];mode=k}; print mode }' file

# 91. Comma-separated unique sorted output
awk '{ a[$1] } END { n=asorti(a,b); s=""; for(i=1;i<=n;i++){if(s!="")s=s",";s=s b[i]} print s }' file
fk  '{ a[$1]++ } END { print join(a, ",") }' file

# 92. Convert key=value config to JSON
awk -F= '{ a[$1]=$2 } END { printf "{"; n=0; for(k in a){if(n++)printf ","; printf "\"%s\":\"%s\"",k,a[k]} print "}" }' cfg
fk  -F= '{ a[$1]=$2 } END { printf "{"; n=0; for(k in a){if(n++)printf ","; printf "\"%s\":\"%s\"",k,a[k]}; print "}" }' cfg

# 93. DNS-style reverse lookup table
awk '{ a[$2]=$1 } END { for(k in a) print k, a[k] }' hosts
fk  '{ a[$2]=$1 } END { for(k in a) print k, a[k] }' hosts   # same

# 94. Sparse matrix to dense with defaults
awk '{ a[$1,$2]=$3 } END { for(r=1;r<=3;r++){for(c=1;c<=3;c++){v=a[r,c];printf "%s ",v?v:0} print""} }' sparse
fk  '{ a[$1,$2]=$3 } END { seq(r,1,3); for(i=1;i<=3;i++){for(j=1;j<=3;j++){v=a[i,j];printf "%s ",v?v:0};print""} }' sparse

# 95. Collect array, compute stats, format report
awk '{ a[NR]=$1;s+=$1 } END { m=s/NR;n=asort(a);for(i=1;i<=n;i++)v+=(a[i]-m)^2; printf "n=%d avg=%.1f med=%.1f sd=%.1f min=%s max=%s\n",n,m,a[int(n/2)+1],sqrt(v/n),a[1],a[n] }' f
fk  '{ a[NR]=$1 } END { printf "n=%d avg=%.1f med=%.1f sd=%.1f min=%s max=%s\n", length(a), mean(a), median(a), stddev(a), min(a), max(a) }' f

# 96. Build histogram of value distribution
awk '{ a[int($1/10)*10]++ } END { for(k in a) { printf "%3d: ", k; for(i=0;i<a[k];i++) printf "#"; print "" } }' file
fk  '{ a[NR]=$1 } END { print histplot(a,10,20,"▇",0,"Histogram","Frequency","yellow") }' file

# 97. Formatted report with headers and totals
awk 'BEGIN{printf "%-20s %10s\n","Name","Amount";printf "%-20s %10s\n","----","------"} {a[$1]+=$2} END{t=0;for(k in a){printf "%-20s %10.2f\n",k,a[k];t+=a[k]};printf "%-20s %10.2f\n","TOTAL",t}' f
fk  'BEGIN{print rpad("Name",20), lpad("Amount",10); print rpad("----",20), lpad("------",10)} {a[$1]+=$2} END{t=0;for(k in a){print rpad(k,20), lpad(sprintf("%.2f",a[k]),10);t+=a[k]};print rpad("TOTAL",20), lpad(sprintf("%.2f",t),10)}' f

# 98. Multi-pass: normalize values to 0-1 range
awk '{ a[NR]=$1 } END { min=a[1];max=a[1]; for(i=2;i<=NR;i++){if(a[i]<min)min=a[i];if(a[i]>max)max=a[i]} for(i=1;i<=NR;i++) printf "%.4f\n",(a[i]-min)/(max-min) }' f
fk  '{ a[NR]=$1 } END { lo=min(a);hi=max(a); for(i=1;i<=length(a);i++) printf "%.4f\n",(a[i]-lo)/(hi-lo) }' f

# 99. Z-score normalization
awk '{ a[NR]=$1;s+=$1 } END { m=s/NR;v=0;for(i=1;i<=NR;i++)v+=(a[i]-m)^2;sd=sqrt(v/NR);for(i=1;i<=NR;i++)printf "%.3f\n",(a[i]-m)/sd }' f
fk  '{ a[NR]=$1 } END { m=mean(a);s=stddev(a); for(i=1;i<=length(a);i++) printf "%.3f\n",(a[i]-m)/s }' f

# 100. Quick one-liner that does everything
awk '{ a[NR]=$1 } END { n=asort(a); delete seen; u=0; for(i=1;i<=n;i++) if(!(a[i] in seen)){seen[a[i]];u++} s=0;for(i=1;i<=n;i++)s+=a[i]; m=s/n; printf "n=%d uniq=%d sum=%g mean=%.2f min=%s max=%s\n",n,u,s,m,a[1],a[n] }' f
fk  '{ a[NR]=$1 } END { b=a; uniq(b); printf "n=%d uniq=%d sum=%g mean=%.2f min=%s max=%s\n", length(a), length(b), sum(a), mean(a), min(a), max(a) }' f
```

---

## The showcase: a program that would be horrific in awk

Monte Carlo poker: deal 100,000 hands, classify each, render a frequency
histogram with stats — in a single fk invocation. Runs in ~1 second.

```sh
fk 'BEGIN {
    srand()
    N = 100000

    for (deal = 1; deal <= N; deal++) {
        # deal 5 unique cards via rejection sampling
        delete seen; n = 0
        while (n < 5) {
            c = int(rand() * 52) + 1
            if (!(c in seen)) { seen[c]; n++; hand[n] = int((c-1)/4) + 1 }
        }

        asort(hand)

        delete rc
        for (i = 1; i <= 5; i++) rc[hand[i]]++

        pairs = 0; trips = 0; quads = 0
        for (r in rc) {
            if (rc[r] == 2) pairs++
            if (rc[r] == 3) trips++
            if (rc[r] == 4) quads++
        }

        # check straight (5 consecutive values)
        straight = (hand[5] - hand[1] == 4 && length(rc) == 5) ? 1 : 0
        if (hand[1]==1 && hand[2]==2 && hand[3]==3 && hand[4]==4 && hand[5]==13)
            straight = 1

        if (quads)               cat = "Four of a Kind"
        else if (trips && pairs) cat = "Full House"
        else if (trips)          cat = "Three of a Kind"
        else if (pairs == 2)     cat = "Two Pair"
        else if (pairs == 1)     cat = "One Pair"
        else if (straight)       cat = "Straight"
        else                     cat = "High Card"

        result[cat]++
    }

    print ""
    print rpad("Hand", 20), lpad("Count", 7), lpad("Pct", 8), "  Distribution"
    print rpad("-", 20, "-"), lpad("-", 7, "-"), lpad("-", 8, "-"), " ", repeat("-", 40)

    split("High Card,One Pair,Two Pair,Three of a Kind,Straight,Full House,Four of a Kind", order, ",")
    for (i = 1; i <= 7; i++) {
        h = order[i]
        n = result[h] + 0
        pct = n / N * 100
        bar = int(pct / 2 + 0.5)
        printf "%s %s %s  %s\n", rpad(h, 20), lpad(n, 7), lpad(sprintf("%.2f%%", pct), 8), repeat("#", bar)
    }
    print ""
    printf "  %d deals simulated.\n", N
}' < /dev/null
```

### Sample output

```
Hand                   Count      Pct   Distribution
-------------------- ------- --------   ----------------------------------------
High Card              41217   41.22%  #####################
One Pair               46286   46.29%  #######################
Two Pair                6950    6.95%  ###
Three of a Kind         4583    4.58%  ##
Straight                 324    0.32%
Full House               421    0.42%
Four of a Kind           219    0.22%

  100000 deals simulated.
```

### The same in awk

The poker program above uses `asort`, `lpad`, `rpad`, `repeat`,
and `length(arr)` — all single calls in fk. The awk equivalent needs:

- `asort` only exists in gawk; BSD awk has no array sort at all
- `printf` width gymnastics instead of `lpad`/`rpad`
- `for` loops to build repeated strings instead of `repeat()`
- ~40% more code, harder to read, slower to write

That's the pitch: **awk-compatible core, modern ergonomics**.

---
---

## Sourced programs: Pement's awk1line.txt & Unix tool equivalents

The programs below are sourced from real references for compatibility
testing, performance comparison, and regression tests.

### Sources

- Eric Pement, *Handy One-Line Scripts for Awk*, v0.28 (2019) — [pement.org/awk/awk1line.txt](https://www.pement.org/awk/awk1line.txt)
- Peter Krumins, *Awk One-Liners Explained* (2008–2009) — [catonmat.net](https://catonmat.net/awk-one-liners-explained-part-one)
- Eric Pement, *Handy One-Line Scripts for Sed* — via catonmat.net
- GNU Awk User's Guide — [gnu.org/software/gawk/manual](https://www.gnu.org/software/gawk/manual/gawk.html)

---

### P. Pement's awk one-liners

#### File spacing

```sh
# P1. Double space a file
awk '1;{print ""}'
fk  '1;{print ""}'

# P2. Double space (alternate)
awk 'BEGIN{ORS="\n\n"};1'
fk  'BEGIN{ORS="\n\n"};1'

# P3. Double space, but no more than one blank between text lines
awk 'NF{print $0 "\n"}'
fk  'NF{print $0 "\n"}'

# P4. Triple space a file
awk '1;{print "\n"}'
fk  '1;{print "\n"}'
```

#### Numbering and calculations

```sh
# P5. Number lines per file (left aligned)
awk '{print FNR "\t" $0}' files*
fk  '{print FNR "\t" $0}' files*

# P6. Number lines across all files
awk '{print NR "\t" $0}' files*
fk  '{print NR "\t" $0}' files*

# P7. Number lines, right-aligned
awk '{printf("%5d : %s\n", NR, $0)}'
fk  '{printf("%5d : %s\n", NR, $0)}'

# P8. Number only non-blank lines
awk 'NF{$0=++a " :" $0};1'
fk  'NF{$0=++a " :" $0};1'

# P9. Count lines (wc -l)
awk 'END{print NR}'
fk  'END{print NR}'

# P10. Sum of fields per line
awk '{s=0; for (i=1; i<=NF; i++) s=s+$i; print s}'
fk  '{s=0; for (i=1; i<=NF; i++) s=s+$i; print s}'

# P11. Sum of all fields in all lines
awk '{for (i=1; i<=NF; i++) s=s+$i}; END{print s+0}'
fk  '{for (i=1; i<=NF; i++) s=s+$i}; END{print s+0}'

# P12. Replace every field with its absolute value
awk '{for (i=1; i<=NF; i++) if ($i < 0) $i = -$i; print }'
fk  '{for (i=1; i<=NF; i++) if ($i < 0) $i = -$i; print }'
# fk alt:
fk  '{for (i=1; i<=NF; i++) $i = abs($i); print}'

# P13. Total number of fields ("words") in all lines
awk '{ total = total + NF }; END {print total}' file
fk  '{ total = total + NF }; END {print total}' file

# P14. Count lines containing "Beth"
awk '/Beth/{n++}; END {print n+0}' file
fk  '/Beth/{n++}; END {print n+0}' file

# P15. Print largest first field and its line
awk '$1 > max {max=$1; maxline=$0}; END{ print max, maxline}'
fk  '$1 > max {max=$1; maxline=$0}; END{ print max, maxline}'

# P16. Print number of fields in each line
awk '{ print NF ":" $0 }'
fk  '{ print NF ":" $0 }'

# P17. Print last field of each line
awk '{ print $NF }'
fk  '{ print $NF }'
# fk alt:
fk  '{ print $-1 }'

# P18. Print last field of last line
awk '{ field = $NF }; END{ print field }'
fk  'END { print $NF }'

# P19. Print lines with more than 4 fields
awk 'NF > 4'
fk  'NF > 4'

# P20. Print lines where last field > 4
awk '$NF > 4'
fk  '$NF > 4'
```

#### String/array creation

```sh
# P21. Create string of 513 spaces
awk 'BEGIN{while (a++<513) s=s " "; print s}'
fk  'BEGIN{print repeat(" ", 513)}'
```

#### Text conversion and substitution

```sh
# P22. Convert DOS newlines (CR/LF) to Unix (LF)
awk '{sub(/\r$/,"")};1'
fk  '{sub(/\r$/,"")};1'

# P23. Delete leading whitespace (ltrim)
awk '{sub(/^[ \t]+/, "")};1'
fk  '{sub(/^[ \t]+/, "")};1'
# fk alt:
fk  '{print ltrim($0)}'

# P24. Delete trailing whitespace (rtrim)
awk '{sub(/[ \t]+$/, "")};1'
fk  '{sub(/[ \t]+$/, "")};1'
# fk alt:
fk  '{print rtrim($0)}'

# P25. Delete both leading and trailing whitespace (trim)
awk '{gsub(/^[ \t]+|[ \t]+$/,"")};1'
fk  '{gsub(/^[ \t]+|[ \t]+$/,"")};1'
# fk alt:
fk  '{print trim($0)}'

# P26. Align text flush right on 79-column width
awk '{printf "%79s\n", $0}' file
fk  '{printf "%79s\n", $0}' file
# fk alt:
fk  '{print lpad($0, 79)}' file

# P27. Substitute first "foo" with "bar" on each line
awk '{sub(/foo/,"bar")}; 1'
fk  '{sub(/foo/,"bar")}; 1'

# P28. Substitute ALL "foo" with "bar" on each line
awk '{gsub(/foo/,"bar")}; 1'
fk  '{gsub(/foo/,"bar")}; 1'

# P29. Substitute only 4th occurrence (gawk only)
gawk '{$0=gensub(/foo/,"bar",4)}; 1'
fk   '{$0=gensub("foo","bar",4)}; 1'

# P30. Substitute on lines containing "baz"
awk '/baz/{gsub(/foo/, "bar")}; 1'
fk  '/baz/{gsub(/foo/, "bar")}; 1'

# P31. Substitute on lines NOT containing "baz"
awk '!/baz/{gsub(/foo/, "bar")}; 1'
fk  '!/baz/{gsub(/foo/, "bar")}; 1'

# P32. Change "scarlet" or "ruby" or "puce" to "red"
awk '{gsub(/scarlet|ruby|puce/, "red")}; 1'
fk  '{gsub(/scarlet|ruby|puce/, "red")}; 1'

# P33. Reverse order of lines (tac)
awk '{a[i++]=$0} END {for (j=i-1; j>=0;) print a[j--] }' file
fk  '{a[NR]=$0} END {for (i=NR; i>=1; i--) print a[i]}' file

# P34. Sort and print login names
awk -F ":" '{print $1 | "sort" }' /etc/passwd
fk  -F: '!/^#/{u[$1]++} END{print u}' /etc/passwd

# P35. Print first 2 fields in reverse order
awk '{print $2, $1}' file
fk  '{print $2, $1}' file

# P36. Delete second field
awk '{ $2 = ""; print }'
fk  '{ $2 = ""; print }'

# P37. Print fields in reverse order
awk '{for (i=NF; i>0; i--) printf("%s ",$i);print ""}' file
fk  '{for (i=NF; i>0; i--) printf("%s ",$i);print ""}' file

# P38. Concatenate every 5 lines with comma
awk 'ORS=NR%5?",":"\n"' file
fk  'ORS=NR%5?",":"\n"' file
```

#### Selective printing

```sh
# P39. Print first 10 lines (head -10)
awk 'NR < 11'
fk  'NR < 11'

# P40. Print first line (head -1)
awk 'NR>1{exit};1'
fk  'NR>1{exit};1'

# P41. Print last 2 lines (tail -2)
awk '{y=x "\n" $0; x=$0};END{print y}'
fk  '{y=x "\n" $0; x=$0};END{print y}'

# P42. Print last line (tail -1)
awk 'END{print}'
fk  'END{print}'

# P43. Print lines matching regex (grep)
awk '/regex/'
fk  '/regex/'

# P44. Print lines NOT matching regex (grep -v)
awk '!/regex/'
fk  '!/regex/'

# P45. Print line where field #5 equals "abc123"
awk '$5 == "abc123"'
fk  '$5 == "abc123"'

# P46. Print line if field #7 matches regex
awk '$7 ~ /^[a-f]/'
fk  '$7 ~ /^[a-f]/'

# P47. Print line before a regex match
awk '/regex/{print x};{x=$0}'
fk  '/regex/{print x};{x=$0}'

# P48. Print line after a regex match
awk '/regex/{getline;print}'
fk  '/regex/{getline;print}'

# P49. Grep for AAA and BBB and CCC (any order)
awk '/AAA/ && /BBB/ && /CCC/'
fk  '/AAA/ && /BBB/ && /CCC/'

# P50. Grep for AAA then BBB then CCC (in order)
awk '/AAA.*BBB.*CCC/'
fk  '/AAA.*BBB.*CCC/'

# P51. Print lines longer than 64 chars
awk 'length > 64'
fk  'length > 64'

# P52. Print from regex to end of file
awk '/regex/,0'
fk  '/regex/,0'

# P53. Print lines 8 to 12
awk 'NR==8,NR==12'
fk  'NR==8,NR==12'

# P54. Print line 52
awk 'NR==52 {print;exit}'
fk  'NR==52 {print;exit}'

# P55. Print between two patterns (inclusive)
awk '/Iowa/,/Montana/'
fk  '/Iowa/,/Montana/'
```

#### Selective deletion

```sh
# P56. Delete all blank lines
awk NF
fk  NF

# P57. Remove consecutive duplicate lines (uniq)
awk 'a != $0; {a=$0}'
fk  'a != $0; {a=$0}'

# P58. Remove all duplicate lines
awk '!a[$0]++'
fk  '!a[$0]++'
```

---

### T. Unix tool equivalents

```sh
# T1. cut -d, -f1,3 — extract CSV fields
cut -d, -f1,3 file
fk  -F, '{print $1, $3}' file

# T2. cut -c1-10 — extract first 10 characters
cut -c1-10 file
fk  '{print substr($0,1,10)}' file

# T3. head -n 5
head -n 5 file
fk  'NR==5{print;exit};1' file

# T4. tail -n 1
tail -n 1 file
fk  'END{print}' file

# T5. wc -l — count lines
wc -l < file
fk  'END{print NR}' file

# T6. wc -w — count words
wc -w < file
fk  '{w+=NF} END{print w}' file

# T7. sort -u (unique lines)
sort -u file
fk  '!seen[$0]++' file

# T8. uniq — remove consecutive duplicates
uniq file
fk  'a!=$0;{a=$0}' file

# T9. uniq -c — count consecutive duplicates
uniq -c file
fk  'a!=$0{if(a!="")print c,a;c=0;a=$0}{c++} END{print c,a}' file

# T10. sort | uniq -c | sort -rn — frequency count
sort file | uniq -c | sort -rn
fk  '{a[$0]++} END{for(k in a) print a[k], k}' file | sort -rn

# T11. grep pattern
grep pattern file
fk  '/pattern/' file

# T12. grep -c — count matches
grep -c pattern file
fk  '/pattern/{n++} END{print n+0}' file

# T13. grep -v — invert match
grep -v pattern file
fk  '!/pattern/' file

# T14. nl — number lines
nl file
fk  '{printf "%6d\t%s\n", NR, $0}' file

# T15. tac — reverse file
tac file
fk  '{a[NR]=$0} END{for(i=NR;i>=1;i--) print a[i]}' file

# T16. rev — reverse each line
rev file
fk  '{print reverse($0)}' file

# T17. paste -sd, — join lines with comma
paste -sd, file
fk  '{a[NR]=$0} END{print join(a,",")}' file

# T18. tr -s ' ' — squeeze whitespace
tr -s ' ' < file
fk  '{$1=$1; print}' file

# T19. tr -d '\r' — strip carriage returns
tr -d '\r' < file
fk  '{sub(/\r$/,"")};1' file

# T20. seq 1 10 — generate sequence
seq 1 10
fk  'BEGIN{seq(a,1,10); print a}' < /dev/null
```

---

### S. Sed equivalents

```sh
# S1. Substitute first occurrence
sed 's/foo/bar/' file
fk  '{sub(/foo/,"bar")};1' file

# S2. Substitute all occurrences
sed 's/foo/bar/g' file
fk  '{gsub(/foo/,"bar")};1' file

# S3. Delete blank lines
sed '/^$/d' file
fk  NF file

# S4. Delete leading whitespace
sed 's/^[ \t]*//' file
fk  '{print ltrim($0)}' file

# S5. Delete trailing whitespace
sed 's/[ \t]*$//' file
fk  '{print rtrim($0)}' file

# S6. Print lines 10-20
sed -n '10,20p' file
fk  'NR>=10 && NR<=20' file

# S7. Print line containing pattern
sed -n '/pattern/p' file
fk  '/pattern/' file

# S8. Delete lines containing pattern
sed '/pattern/d' file
fk  '!/pattern/' file

# S9. Number each line
sed = file | sed 'N;s/\n/\t/'
fk  '{printf "%d\t%s\n", NR, $0}' file

# S10. Reverse order of lines (tac)
sed '1!G;h;$!d' file
fk  '{a[NR]=$0} END{for(i=NR;i>=1;i--) print a[i]}' file
```

---

### C. Classic two-file awk idioms

```sh
# C1. Lookup join — enrich data from reference file
awk 'NR==FNR{price[$1]=$2; next} {print $0, price[$1]+0}' prices.txt orders.txt
fk  'NR==FNR{price[$1]=$2; next} {print $0, price[$1]+0}' prices.txt orders.txt

# C2. Anti-join — IDs in data but not in blocklist
awk 'NR==FNR{skip[$1]; next} !($1 in skip)' blocklist.txt data.txt
fk  'NR==FNR{skip[$1]++; next} !($1 in skip)' blocklist.txt data.txt

# C3. Semi-join — keep data rows whose key appears in keyfile
awk 'NR==FNR{keep[$1]; next} $1 in keep' keys.txt data.txt
fk  'NR==FNR{keep[$1]++; next} $1 in keep' keys.txt data.txt

# C4. Diff — lines in file1 not in file2
awk 'NR==FNR{a[$0]; next} !($0 in a)' file2 file1
fk  'NR==FNR{a[$0]++; next} !($0 in a)' file2 file1

# C5. Update — overwrite values from second file
awk 'NR==FNR{a[$1]=$2; next} {if($1 in a) $2=a[$1]; print}' updates.txt master.txt
fk  'NR==FNR{a[$1]=$2; next} {if($1 in a) $2=a[$1]; print}' updates.txt master.txt
```

---

### D. fk-only programs (no direct awk equivalent)

```sh
# D1. Median of a column
fk '{a[NR]=$1} END{print median(a)}' file

# D2. Standard deviation
fk '{a[NR]=$1} END{print stddev(a)}' file

# D3. Percentile (p95)
fk '{a[NR]=$1} END{print p(a,95)}' file

# D4. Interquartile mean
fk '{a[NR]=$1} END{print iqm(a)}' file

# D5. Variance
fk '{a[NR]=$1} END{print variance(a)}' file

# D6. Full summary in one pass
fk '{a[NR]=$1} END{printf "n=%d mean=%.2f med=%.2f sd=%.2f min=%s max=%s\n", length(a), mean(a), median(a), stddev(a), min(a), max(a)}' file

# D7. Shuffle all lines
fk '{a[NR]=$0} END{shuf(a); print a}' file

# D8. Random sample of 10 lines (reservoir sampling)
fk '{a[NR]=$0} END{samp(a,10); print a}' file

# D9. Generate integer sequence
fk 'BEGIN{seq(a,1,100); print a}' < /dev/null

# D10. Deduplicate array values
fk '{a[NR]=$1} END{uniq(a); print a}' file

# D11. Set difference on arrays
fk 'NR==FNR{a[$0]++;next}{b[$0]++} END{diff(a,b); for(k in a) print k}' file1 file2

# D12. Set intersection on arrays
fk 'NR==FNR{a[$0]++;next}{b[$0]++} END{inter(a,b); for(k in a) print k}' file1 file2

# D13. Set union on arrays
fk 'NR==FNR{a[$0]++;next}{b[$0]++} END{union(a,b); for(k in a) print k}' file1 file2

# D14. Invert key/value pairs
fk '{a[$1]=$2} END{inv(a); for(k in a) print k, a[k]}' file

# D15. Remove empty/zero entries from array
fk '{a[NR]=$1} END{tidy(a); print a}' file

# D16. Trim whitespace (builtin)
fk '{print trim($0)}' file

# D17. Left/right pad strings
fk '{print lpad($1,10), rpad($2,20)}' file

# D18. Repeat a string N times
fk 'BEGIN{print repeat("=-",40)}'

# D19. Reverse a string (unicode-aware)
fk '{print reverse($0)}' file

# D20. Named column access with header mode
fk -H '{print $"first-name", $"last-name"}' data.csv

# D21. CSV auto-detect from extension
fk -H '{rev[$region]+=$revenue} END{for(r in rev) print r, rev[r]}' sales.csv

# D22. JSON Lines input with jpath
fk -i json '{print jpath($0, ".user.name")}' data.jsonl

# D23. Slurp file into array
fk 'BEGIN{n=slurp("words.txt",w); print n, "lines"}' < /dev/null

# D24. Parquet input
fk -i parquet -H '{print $name, $age}' people.parquet

# D25. Transparent decompression
fk '{print}' data.csv.gz

# D26. Auto-describe format and schema
fk --describe data.csv

# D27. Match with capture groups
fk '{if(match($0, /(\w+)=(\d+)/, m)) print m[1], m[2]}' config.txt

# D28. Bitwise operations
fk 'BEGIN{print and(0xFF, 0x0F), or(0xA0, 0x05), xor(0xFF, 0x0F)}'

# D29. Negative field indexing
fk '{print $-1, $-2}' file

# D30. Computed regex from variable
fk -v pat="error|warn" '$0 ~ pat' log.txt
```
