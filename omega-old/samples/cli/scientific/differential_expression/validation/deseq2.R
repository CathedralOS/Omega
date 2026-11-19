# Independent, pinned likelihood/Wald reference. Moment dispersion supplied;
# this intentionally does not call DESeq() or its dispersion shrinkage pipeline.
suppressPackageStartupMessages(library(DESeq2))
stopifnot(as.character(packageVersion('DESeq2')) == '1.46.0')
arguments <- commandArgs(trailingOnly=TRUE)
root <- normalizePath(arguments[1])
output <- arguments[2]
dir.create(output, recursive=TRUE, showWarnings=FALSE)
for (file in list.files(file.path(root,'data'), pattern='\\.stdin$', full.names=TRUE)) {
  v <- tryCatch(scan(file,quiet=TRUE),error=function(e)numeric())
  if(length(v)!=54 || any(v!=floor(v))) next
  group <- v[1:6]; y <- matrix(v[7:54],nrow=8,byrow=TRUE)
  if(any(!group %in% c(0,1)) || sum(group)<2 || sum(group)>4 || any(y<0|y>1000)) next
  positive <- apply(y,1,function(r)all(r>0))
  if(!any(positive)) next
  sf <- exp(apply(log(y[positive,,drop=FALSE])-rowMeans(log(y[positive,,drop=FALSE])),2,median))
  norm <- sweep(y,2,sf,'/')
  a <- rowMeans(norm[,group==0,drop=FALSE]); b <- rowMeans(norm[,group==1,drop=FALSE])
  keep <- rowSums(y)>=10 & a>0 & b>0
  if(!any(keep)) next
  means <- sapply(group,function(g)if(g==0)a else b)
  alpha <- pmin(10,pmax(1e-6,rowSums((norm-means)^2-sweep(means,2,sf,'/'))/(4/6*rowSums(means^2))))
  d <- DESeqDataSetFromMatrix(y[keep,,drop=FALSE],data.frame(group=factor(group)),~group)
  sizeFactors(d) <- sf
  dispersions(d) <- alpha[keep]
  d <- nbinomWaldTest(d,betaPrior=FALSE,minmu=1e-6,betaTol=1e-10,maxit=1000,useOptim=TRUE)
  result <- results(d,contrast=c('group','1','0'),independentFiltering=FALSE,cooksCutoff=FALSE)
  result$gene <- which(keep)-1
  result$dispersion <- dispersions(d)
  fitted <- coef(d)
  result$mean0 <- 2^fitted[,1]
  result$mean1 <- 2^(fitted[,1]+fitted[,2])
  result$logLikelihood <- rowSums(matrix(dnbinom(counts(d),mu=assays(d)[['mu']],size=1/dispersions(d),log=TRUE),nrow=nrow(d)))
  write.csv(as.data.frame(result),file.path(output,paste0(tools::file_path_sans_ext(basename(file)),'.csv')),row.names=FALSE)
  write.table(sf,file.path(output,paste0(tools::file_path_sans_ext(basename(file)),'.factors')),row.names=FALSE,col.names=FALSE)
}
writeLines(sub("[[:space:]]+$", "", capture.output(sessionInfo())),file.path(output,'session.txt'))
